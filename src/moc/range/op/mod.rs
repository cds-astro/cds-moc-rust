//! This module contains the structures used to perform operations on Range MOC iterators.

pub mod check;
pub mod convert;
pub mod merge;

pub mod degrade;
pub mod not; // <=> complement

pub mod and; // <=> intersection
pub mod minus; // <=> mocpy difference = Aladin Soustracction
pub mod or; // <=> union
pub mod xor; // <=> Aladin Difference

pub mod multi_op;

pub mod overlap;
