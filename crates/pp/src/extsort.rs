//! Module containing a generic external sort algorithm.

use std::{
  error::Error,
  fmt::Debug,
  fs::{self, File, create_dir_all},
  io::{BufReader, BufWriter, Error as IoError, Read, Write},
  marker::PhantomData,
  path::{Path, PathBuf},
  thread,
};

use crossbeam::channel::bounded;
use itertools::Itertools;
use log::warn;
use rayon::prelude::*;

/// Prefix of external sort temporary files
const TMP_FILE_PREFIX: &str = ".extsort_chunk";

/// Represents the entry of a list that can be sorted using an external sort.
/// This entry is possibly read and written several times.
/// Thus, this representation must only ensure that the entry is fast to
/// compare, read and write.
/// It does not have to represent the full complexity of the entry structure.
/// This representation may be obtained mapping the full complex entry, and the full complex
/// entry may be obtained back mapping this "simplified" representation.
/// Such mapping is out of the scope of this library.
pub trait ExtSortEntry: 'static + Sized + PartialOrd + Ord + Send + Debug {
  /// # WARNING
  /// * For performances, you probably want to pass a `BufWriter`.
  fn write<W: Write>(self, writer: &mut W) -> Result<usize, IoError>;
  /// # WARNING
  /// * For performances, you probably want to pass a `BufRead`.
  fn read<R: Read>(reader: &mut R) -> Result<Self, IoError>;
}

/// Parameters of the external sort algorithm.
struct ExtSortParams {
  /// Number of entries (rows) processed at the same time in memory.
  /// It is also the the number of entries (rows) in the first created temporary files.
  chunk_size: usize,
  /// 'k' value of the external k-way merge sort, i.e. maximum number of temporary files merge
  /// at the same time.
  kway: usize,
  /// Temporary directory containing temporary files
  tmp_dir: PathBuf,
}
impl Default for ExtSortParams {
  fn default() -> Self {
    Self {
      chunk_size: 50_000,
      kway: 7,
      tmp_dir: ".ext_sort_tmp".into(),
    }
  }
}

/// Builder to construct a `ExternalSort` structure.
pub struct ExtSortBuilder {
  args: ExtSortParams,
}
impl ExtSortBuilder {
  /// Create a new e builder, with defaut parameter values.
  pub fn new() -> Self {
    Self::default()
  }
  /// Sets the size of a chunck, i.e. the number of entries sorted at once in memory,
  /// and the number of entries per initial temporary files.
  pub fn set_chunk_size(mut self, n_entries: usize) -> Self {
    self.args.chunk_size = n_entries;
    self
  }
  /// Sets the 'k' value of the external k-way merge.
  /// I.e. the number of temporary files that are merge at once to produce either
  /// an intermediary temporary file or the final result.
  pub fn set_kway(mut self, k: usize) -> Self {
    self.args.kway = k;
    self
  }
  /// Sets the temporary directory, i.e. the directory that will contains temporary files.
  pub fn set_tmp_dir<P: AsRef<Path>>(mut self, path: P) -> Self {
    self.args.tmp_dir = path.as_ref().into();
    self
  }
  /// Build the `ExternalSort` structure.
  pub fn build(self) -> ExtSort {
    ExtSort { args: self.args }
  }
}
impl Default for ExtSortBuilder {
  fn default() -> Self {
    Self {
      args: Default::default(),
    }
  }
}

pub struct ExtSort {
  args: ExtSortParams,
}
impl ExtSort {
  /// Returns the total number of entries, and a sorted iterator and those entries.
  pub fn external_sort<E, R, I>(
    self,
    it: I,
  ) -> Result<(u64, impl Iterator<Item = R>), Box<dyn Error>>
  where
    E: Error + Send + 'static,
    R: ExtSortEntry,
    I: Iterator<Item = Result<R, E>> + Send + 'static,
  {
    type Chunk<R> = Vec<R>;

    // Communication channels between reader thread, sort thread and writer thread
    let (sender_read, receiver_sort) = bounded::<Chunk<R>>(0); // replace 0 by 1 for buffering (+1 chunk i memory)
    let (sender_sort, receiver_write) = bounded::<Chunk<R>>(0); // replace 0 by 1 for buffering (+1 chunk in memory)

    // Reader thread (producer)
    let reader = thread::spawn(move || -> Result<(), E> {
      for chunk in &it.chunks(self.args.chunk_size) {
        let chunk = chunk.collect::<Result<Chunk<R>, E>>()?;
        sender_read
          .send(chunk)
          .expect("Sorter thread terminated (probably due to an error in writer thread)!!");
      }
      // Close the channel, otherwise sink will never exit the for-loop
      drop(sender_read);
      Ok(())
    });
    // Sorter thread
    let sorter = thread::spawn(move || {
      for mut chunk in receiver_sort.iter() {
        chunk.par_sort();
        sender_sort
          .send(chunk)
          .expect("Writer thread terminated (probably due to an error while writing)");
      }
      // Close the channel, otherwise sink will never exit the for-loop
      drop(sender_sort);
    });
    // Writer thread
    let writer = thread::spawn(move || -> Result<TmpDir<R>, IoError> {
      let mut tmp_dir = TmpDir::new(self.args.tmp_dir)?;
      while let Ok(sorted_chunk) = receiver_write.recv() {
        tmp_dir.write_new_tmp_file(sorted_chunk.into_iter())?;
      }
      Ok(tmp_dir)
    });

    // Join all threads to be terminated
    let tmp_dir = writer.join().unwrap()?;
    sorter.join().unwrap();
    reader
      .join()
      .unwrap()
      .map_err(|e| format!("Read thread input error: {:?}", e))?;
    // Count the total number of entries
    let n_entries = tmp_dir.n_entries_in_tmp_files();
    // Now, merge tmp files
    tmp_dir
      .reduce_to_k_files_and_return_merged_it(self.args.kway)
      .map_err(|e| e.into())
      .map(|it| (n_entries, it))
  }
}

/// Represent the temporary directory containing the sorted temporary files.
struct TmpDir<R: ExtSortEntry> {
  path: PathBuf,
  level: u8,
  is_last_level: bool,
  files: Vec<(u32, usize)>, // chunk index, count
  _phantom: PhantomData<R>,
}
impl<R: ExtSortEntry> TmpDir<R> {
  fn new(path: PathBuf) -> Result<Self, IoError> {
    if !path.is_dir() {
      create_dir_all(&path)
        .map_err(|e| IoError::other(format!("Unable to create tmp directory: {:?}", e)))
    } else {
      Ok(())
    }
    .map(|()| Self {
      path,
      level: 0,
      is_last_level: false,
      files: Default::default(),
      _phantom: PhantomData,
    })
  }
  fn n_entries_in_tmp_files(&self) -> u64 {
    self.files.iter().map(|(_, c)| *c as u64).sum::<u64>()
  }
  fn write_new_tmp_file<I>(&mut self, sorted_entries: I) -> Result<(), IoError>
  where
    I: Iterator<Item = R>,
  {
    let idx = self.files.len() as u32;
    TmpFileWriter::new(&self.path, self.level, idx)
      .write(sorted_entries)
      .and_then(|n_written| {
        self.files.push((idx, n_written));
        Ok(())
      })
  }
  fn next_level(&self) -> Self {
    Self {
      path: self.path.clone(),
      level: self.level + 1,
      is_last_level: false,
      files: Default::default(),
      _phantom: PhantomData,
    }
  }
  /// Recursive function working level by level till the remaining number of temporary file is
  /// lower or equal to `k`
  fn reduce_to_k_files_and_return_merged_it(
    mut self,
    k: usize,
  ) -> Result<impl Iterator<Item = R>, IoError> {
    if self.files.len() > k {
      let mut next_level_dir = self.next_level();
      // Reduce by k-way merge using itertools
      // TODO: introduce multi-threading here? (Maybe ok for SSDs, but I doubt it is for HDDs!)
      for chunk in &self.files.drain(..).chunks(k) {
        // Merge k tmp files into a new file
        let iterators: Vec<_> = chunk
          .map(|(idx, n_entries)| TmpFileReader::new(&self.path, self.level, idx, n_entries))
          .collect::<Result<Vec<_>, IoError>>()?;
        next_level_dir.write_new_tmp_file(iterators.into_iter().kmerge())?;
      }
      // Recurse
      next_level_dir.reduce_to_k_files_and_return_merged_it(k)
    } else {
      self.is_last_level = true;
      let iterators: Vec<_> = self
        .files
        .drain(..)
        .map(|(idx, n_entries)| TmpFileReader::new(&self.path, self.level, idx, n_entries))
        .collect::<Result<Vec<_>, IoError>>()?;
      Ok(iterators.into_iter().kmerge())
    }
  }
}

/* Can't work, because the struct is dropped when the kmerge iterator is built, hence
files are not yet removed!
impl<R: ExtSortEntry> Drop for TmpDir<R> {
  fn drop(&mut self) {
    if self.is_last_level {
      fs::remove_dir(&self.path).unwrap_or_else(|e| {
        warn!(
          "Unable to remove the temporary directory: {}.Error: {:?}",
          self.path.display(),
          e
        )
      });
    }
  }
}*/

/// Structure used to write temporary files.
struct TmpFileWriter {
  path: PathBuf,
}
impl TmpFileWriter {
  fn new(dir: &PathBuf, level: u8, index: u32) -> Self {
    let filename = format!("{}_l{}i{}", TMP_FILE_PREFIX, level, index);
    Self {
      path: dir.join(filename),
    }
  }
  /// Returns the number of entries written in the temporary file.
  /// # WARNING
  /// * The input iterator **must be** sorted (this is not checked!!)
  fn write<I>(self, sorted_it: I) -> Result<usize, IoError>
  where
    I: Iterator,
    I::Item: ExtSortEntry,
  {
    // We want to fail if a temporary file already exists, because 2 process may have been launched
    // at the same time, in which case they will interfere.
    let mut w = File::create_new(&self.path).map(BufWriter::new)?;
    let mut count = 0_usize;
    for entry in sorted_it {
      entry.write(&mut w)?;
      count += 1;
    }
    w.flush().map(|()| count)
  }
}

/// Struct used to read the content of a temporary file (getting an iterator from it).
/// # WARNING
/// `Drop` is implemented on this struct so that the file is automatically removed!
struct TmpFileReader<R: ExtSortEntry> {
  path: PathBuf,
  reader: BufReader<File>,
  n_entries: usize,
  n_read: usize,
  _phantom: PhantomData<R>,
}
impl<R: ExtSortEntry> TmpFileReader<R> {
  fn new(dir: &PathBuf, level: u8, index: u32, n_entries: usize) -> Result<Self, IoError> {
    let filename = format!("{}_l{}i{}", TMP_FILE_PREFIX, level, index);
    let path = dir.join(filename);
    File::open(&path).map(move |file| Self {
      path,
      reader: BufReader::new(file),
      n_entries,
      n_read: 0,
      _phantom: PhantomData,
    })
  }
}
impl<R: ExtSortEntry> Iterator for TmpFileReader<R> {
  type Item = R;

  fn size_hint(&self) -> (usize, Option<usize>) {
    let n_remaining = self.n_entries - self.n_read;
    (n_remaining, Some(n_remaining))
  }

  fn next(&mut self) -> Option<Self::Item> {
    if self.n_read < self.n_entries {
      self.n_read += 1;
      let entry = R::read(&mut self.reader).unwrap_or_else(|e| {
        panic!(
          "Error reading entry in file {}: {:?}",
          self.path.display(),
          &e
        )
      });
      Some(entry)
    } else {
      None
    }
  }
}
impl<R: ExtSortEntry> Drop for TmpFileReader<R> {
  fn drop(&mut self) {
    fs::remove_file(&self.path).unwrap_or_else(|e| {
      warn!(
        "Unable to remove file: {}.Error: {:?}",
        self.path.display(),
        e
      )
    });
    // Check the parent directory
    if let Some(parent) = self.path.parent() {
      if let Ok(mut it) = fs::read_dir(parent) {
        // If no more file in it...
        if it.next().is_none() {
          // ... then remove it!
          fs::remove_dir(&parent).unwrap_or_else(|e| {
            warn!(
              "Unable to remove the temporary directory: {}.Error: {:?}",
              self.path.display(),
              e
            )
          });
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::cmp::{Eq, Ord, Ordering, PartialOrd};

  use super::*;

  #[test]
  fn test_external_sort() {
    // Create external sortable struct
    #[derive(Debug, PartialOrd, PartialEq)]
    struct F64(f64);
    impl Eq for F64 {}
    impl Ord for F64 {
      fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
      }
    }
    impl ExtSortEntry for F64 {
      fn write<W: Write>(self, writer: &mut W) -> Result<usize, IoError> {
        writer
          .write_all(&self.0.to_le_bytes())
          .map(|()| size_of::<f64>())
      }

      fn read<R: Read>(reader: &mut R) -> Result<Self, IoError> {
        let mut val = [0_u8; size_of::<f64>()];
        reader
          .read_exact(&mut val)
          .map(|()| F64(f64::from_le_bytes(val)))
      }
    }
    // Create data
    let entries = [0.5_f64, 1.0, 0.1, 0.6, 0.8, 0.3, 0.4, 5.0, 7.0, 0.001];
    let mut expected = entries.into_iter().map(F64).collect::<Vec<F64>>();
    expected.sort();
    // Sort and check result
    match ExtSortBuilder::new()
      .set_chunk_size(2)
      .build()
      .external_sort(entries.into_iter().map(|val| Ok::<_, IoError>(F64(val))))
      /*.external_sort(entries.into_iter().map(|val| {
        if val == 0.5 {
          Err::<_, IoError>(IoError::other("Error toto".to_string()))
        } else {
          Ok::<_, IoError>(F64(val))
        }
      }))*/
      {
      Ok((_n_entrires, iter)) => {
        let actual = iter.collect::<Vec<F64>>();
        assert_eq!(actual, expected)
      }
      Err(err) => {
        eprintln!("External sort error: {:?}", err);
        assert!(false)
      }
    }
  }
}
