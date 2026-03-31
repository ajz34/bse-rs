//! Helper functions for formatting basis set output.
//!
//! Provides utilities for formatting coefficient matrices with
//! proper alignment and scientific notation.

use crate::prelude::*;

/// Calculate left padding for each column to align decimal points.
///
/// # Arguments
///
/// * `column` - Column of numbers as strings
/// * `point_place` - Desired position of the decimal point
fn determine_leftpad(column: &[String], point_place: usize) -> Vec<usize> {
    column
        .iter()
        .map(|s| {
            let ndigit_left = s.find('.').unwrap_or(0);
            (point_place as i32 - 1 - ndigit_left as i32).max(0) as usize
        })
        .collect()
}

/// Format a matrix with aligned decimal points.
///
/// Writes a coefficient matrix with proper spacing so that all decimal
/// points line up vertically.
///
/// # Arguments
///
/// * `mat` - Matrix to format (coefficients as strings)
/// * `point_places` - Desired decimal point positions for each column
/// * `convert_exp` - If true, convert 'e' to 'D' for Fortran compatibility
///
/// # Returns
///
/// A formatted string representation of the matrix.
pub fn write_matrix(mat: &[Vec<String>], point_places: &[usize], convert_exp: bool) -> String {
    // Padding for the whole matrix
    let pad = mat.iter().zip(point_places).map(|(c, &p)| determine_leftpad(c, p)).collect_vec();

    // Use the transposes (easier to write out by row)
    let pad = misc::transpose_matrix(&pad);
    let mat = misc::transpose_matrix(mat);

    let mut lines: Vec<String> = vec![];
    for (r, row) in mat.iter().enumerate() {
        let mut line = String::new();
        for (c, s) in row.iter().enumerate() {
            let mut sp = pad[r][c] - line.len();
            // ensure at least one space, except for the beginning of the line
            if c > 0 {
                sp = sp.max(1);
            }
            line.push_str(&" ".repeat(sp));
            line.push_str(s);
        }
        lines.push(line);
    }

    let mut lines = lines.join("\n");
    if convert_exp {
        lines = lines.replace("E", "D").replace("e", "D");
    }
    lines
}
