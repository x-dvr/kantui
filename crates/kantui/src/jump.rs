//! Jump-label generator for the `gw` quick-jump flow.
//!
//! Labels are two lowercase ASCII letters. We walk the visible tasks in
//! column-major order and hand them out `aa`, `ab`, ..., `az`, `ba`, ...
//! That gives 676 distinct labels, more than enough for any realistic board
//! that fits on a terminal.

use crate::app::{App, JumpLabel};

const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

/// Emit a [`JumpLabel`] for every visible task in `app`. Order is
/// column-first, task-second — the first label goes to the top of column 0,
/// the next to the task below it, and so on.
#[must_use]
pub fn generate(app: &App) -> Vec<JumpLabel> {
    let mut labels = Vec::new();
    let mut cursor = 0usize;
    for column in 0..app.board.tasks_by_state.len() {
        let visible = app.visible_tasks(column);
        for visible_index in 0..visible.len() {
            let Some(label) = nth_label(cursor) else {
                return labels;
            };
            labels.push(JumpLabel {
                column,
                visible_index,
                label,
            });
            cursor += 1;
        }
    }
    labels
}

fn nth_label(n: usize) -> Option<[char; 2]> {
    let base = ALPHABET.len();
    if n >= base * base {
        return None;
    }
    let first = ALPHABET[n / base] as char;
    let second = ALPHABET[n % base] as char;
    Some([first, second])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_label_wraps_through_alphabet() {
        assert_eq!(nth_label(0), Some(['a', 'a']));
        assert_eq!(nth_label(1), Some(['a', 'b']));
        assert_eq!(nth_label(25), Some(['a', 'z']));
        assert_eq!(nth_label(26), Some(['b', 'a']));
        assert_eq!(nth_label(26 * 26 - 1), Some(['z', 'z']));
        assert_eq!(nth_label(26 * 26), None);
    }
}
