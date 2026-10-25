//! The `java.util.Arrays` methods, and `clone()`, that the translator knows for arrays of primitives
//! and strings. Each takes the array (and its other arguments) as its first arguments.

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArraysOp {
    /// `Arrays.equals(a, b)`: null arrays are equal only to each other.
    Equals,
    /// `Arrays.hashCode(a)`.
    HashCode,
    /// `Arrays.sort(a)` for an array of strings, which may hold null.
    SortAll,
    /// `Arrays.sort(a, from, to)`.
    SortRange,
    /// `Arrays.fill(a, from, to, v)`.
    FillRange,
    /// `Arrays.copyOfRange(a, from, to)`.
    CopyOfRange,
    /// `Arrays.binarySearch(a, key)`.
    BinarySearch,
    /// `Arrays.binarySearch(a, from, to, key)`.
    BinarySearchRange,
    /// `Arrays.deepToString(a)` for an array of arrays of primitives or strings.
    DeepToString,
    /// `a.clone()`: a shallow copy, whose rows are shared when the elements are arrays.
    Clone,
}

impl ArraysOp {
    /// Whether the method takes a range `from`, `to` of the array, which throws
    /// `IllegalArgumentException` when `from > to` and `ArrayIndexOutOfBoundsException` when it is
    /// outside the array.
    pub fn ranged(self) -> bool {
        matches!(
            self,
            ArraysOp::SortRange | ArraysOp::FillRange | ArraysOp::CopyOfRange | ArraysOp::BinarySearchRange
        )
    }
}
