//! Performance Utilities
//!
//! This module contains utilities to improve performance and reduce memory allocations
//! throughout the application.

use std::borrow::Cow;

/// Efficiently concatenate strings with minimal allocations.
///
/// `StringBuilder` is designed to reduce memory allocations when building strings
/// from multiple parts. It pre-allocates capacity based on an estimated final size
/// and collects all parts before performing a single allocation for the final string.
///
/// # Examples
///
/// ```
/// use rona::performance::StringBuilder;
///
/// let mut builder = StringBuilder::with_capacity(50);
/// builder.push("Hello");
/// builder.push_str(" ");
/// builder.push("World");
/// builder.push("!");
///
/// let result = builder.build();
/// assert_eq!(result, "Hello World!");
/// ```
pub struct StringBuilder {
    parts: Vec<String>,
    estimated_size: usize,
}

impl StringBuilder {
    /// Create a new `StringBuilder` with an estimated final size.
    ///
    /// The `estimated_size` parameter helps pre-allocate the appropriate capacity
    /// for the final string, reducing the need for reallocations during building.
    ///
    /// # Arguments
    ///
    /// * `estimated_size` - The estimated size in bytes of the final string
    ///
    /// # Examples
    ///
    /// ```
    /// use rona::performance::StringBuilder;
    ///
    /// // Create a builder expecting roughly 100 characters
    /// let builder = StringBuilder::with_capacity(100);
    /// ```
    #[must_use]
    pub const fn with_capacity(estimated_size: usize) -> Self {
        Self {
            parts: Vec::new(),
            estimated_size,
        }
    }

    /// Add a string part to the builder.
    ///
    /// This method accepts any type that can be converted into a `String`,
    /// providing flexibility for different input types.
    ///
    /// # Arguments
    ///
    /// * `s` - Any value that implements `Into<String>`
    ///
    /// # Examples
    ///
    /// ```
    /// use rona::performance::StringBuilder;
    ///
    /// let mut builder = StringBuilder::with_capacity(20);
    /// builder.push("Hello");
    /// builder.push(String::from(" World"));
    /// builder.push(42.to_string());
    /// ```
    pub fn push<S: Into<String>>(&mut self, s: S) {
        self.parts.push(s.into());
    }

    /// Add a string slice to the builder.
    ///
    /// This is a convenience method for adding string slices without
    /// explicit conversion. Note that this still requires allocation
    /// to convert the `&str` to `String`.
    ///
    /// # Arguments
    ///
    /// * `s` - A string slice to add
    ///
    /// # Examples
    ///
    /// ```
    /// use rona::performance::StringBuilder;
    ///
    /// let mut builder = StringBuilder::with_capacity(20);
    /// builder.push_str("Hello");
    /// builder.push_str(" World");
    /// ```
    pub fn push_str(&mut self, s: &str) {
        self.parts.push(s.to_string());
    }

    /// Build the final string from all accumulated parts.
    ///
    /// This method consumes the `StringBuilder` and returns the final concatenated
    /// string. It calculates the total length needed and pre-allocates the
    /// appropriate capacity to minimize allocations.
    ///
    /// # Returns
    ///
    /// A `String` containing all the concatenated parts
    ///
    /// # Examples
    ///
    /// ```
    /// use rona::performance::StringBuilder;
    ///
    /// let mut builder = StringBuilder::with_capacity(20);
    /// builder.push("Hello");
    /// builder.push_str(" ");
    /// builder.push("World");
    ///
    /// let result = builder.build();
    /// assert_eq!(result, "Hello World");
    /// ```
    #[must_use]
    pub fn build(self) -> String {
        let total_len: usize = self.parts.iter().map(String::len).sum();
        let mut result = String::with_capacity(total_len.max(self.estimated_size));

        for part in self.parts {
            result.push_str(&part);
        }

        result
    }
}

/// Efficiently format file paths without unnecessary allocations.
///
/// This function intelligently handles path concatenation by borrowing
/// the input when possible (when base is empty or file is absolute)
/// and only allocating when concatenation is actually needed.
///
/// # Arguments
///
/// * `base` - The base directory path
/// * `file` - The file path to append
///
/// # Returns
///
/// A `Cow<str>` that either borrows the input file path or owns
/// a newly allocated concatenated path
///
/// # Examples
///
/// ```
/// use rona::performance::format_file_path;
/// use std::borrow::Cow;
///
/// // Borrows when base is empty
/// assert_eq!(format_file_path("", "file.txt"), Cow::Borrowed("file.txt"));
///
/// // Borrows when file is absolute
/// assert_eq!(format_file_path("base", "/absolute/file.txt"),
///            Cow::Borrowed("/absolute/file.txt"));
///
/// // Allocates when concatenation is needed
/// assert_eq!(format_file_path("base", "file.txt"),
///            Cow::Owned("base/file.txt".to_string()));
///
/// // Handles trailing slashes correctly
/// assert_eq!(format_file_path("base/", "file.txt"),
///            Cow::Owned("base/file.txt".to_string()));
/// ```
#[must_use]
pub fn format_file_path<'a>(base: &'a str, file: &'a str) -> Cow<'a, str> {
    if base.is_empty() || file.starts_with('/') {
        Cow::Borrowed(file)
    } else {
        Cow::Owned(format!("{}/{}", base.trim_end_matches('/'), file))
    }
}

/// Batch process items to reduce system call overhead.
///
/// This function processes items in chunks of the specified batch size,
/// which can significantly reduce overhead when dealing with system calls
/// or other expensive operations that benefit from batching.
///
/// # Type Parameters
///
/// * `T` - The type of items to process
/// * `F` - The processor function type
/// * `R` - The type of results returned by the processor
///
/// # Arguments
///
/// * `items` - A slice of items to process
/// * `batch_size` - The maximum number of items to process in each batch
/// * `processor` - A function that processes a batch of items and returns results
///
/// # Returns
///
/// A `Vec<R>` containing all results from processing all batches
///
/// # Examples
///
/// ```
/// use rona::performance::batch_process;
///
/// let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
///
/// // Process in batches of 3, squaring each number
/// let results = batch_process(&numbers, 3, |batch| {
///     batch.iter().map(|&x| x * x).collect()
/// });
///
/// assert_eq!(results, vec![1, 4, 9, 16, 25, 36, 49, 64, 81, 100]);
/// ```
///
/// # Performance Notes
///
/// The optimal batch size depends on the specific use case:
/// - For file operations: 50-100 items per batch
/// - For network operations: 10-50 items per batch
/// - For CPU-intensive operations: Consider the number of CPU cores
pub fn batch_process<T, F, R>(items: &[T], batch_size: usize, mut processor: F) -> Vec<R>
where
    F: FnMut(&[T]) -> Vec<R>,
{
    let mut results = Vec::with_capacity(items.len());

    for chunk in items.chunks(batch_size) {
        let mut batch_results = processor(chunk);
        results.append(&mut batch_results);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_builder() {
        let mut builder = StringBuilder::with_capacity(20);
        builder.push("Hello");
        builder.push_str(" ");
        builder.push("World");

        assert_eq!(builder.build(), "Hello World");
    }

    #[test]
    fn test_format_file_path() {
        assert_eq!(format_file_path("", "file.txt"), "file.txt");
        assert_eq!(format_file_path("base", "file.txt"), "base/file.txt");
        assert_eq!(format_file_path("base/", "file.txt"), "base/file.txt");
        assert_eq!(
            format_file_path("base", "/absolute/file.txt"),
            "/absolute/file.txt"
        );
    }
}
