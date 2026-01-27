#![allow(dead_code)]

use crate::editor::buffer::TextBuffer;
use crate::editor::cursor::CursorPosition;

const MAX_HISTORY_SIZE: usize = 100;

#[derive(Clone)]
pub struct HistoryEntry {
    pub buffer: TextBuffer,
    pub cursor: CursorPosition,
}

pub struct History {
    /// Undo stack (past states)
    undo_stack: Vec<HistoryEntry>,
    /// Redo stack (future states after undo)
    redo_stack: Vec<HistoryEntry>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Save current state to undo stack (call before making changes)
    pub fn push(&mut self, buffer: &TextBuffer, cursor: CursorPosition) {
        // Clear redo stack when new changes are made
        self.redo_stack.clear();

        self.undo_stack.push(HistoryEntry {
            buffer: buffer.snapshot(),
            cursor,
        });

        // Limit history size
        if self.undo_stack.len() > MAX_HISTORY_SIZE {
            self.undo_stack.remove(0);
        }
    }

    /// Undo: restore previous state, save current to redo stack
    pub fn undo(
        &mut self,
        current_buffer: &TextBuffer,
        current_cursor: CursorPosition,
    ) -> Option<HistoryEntry> {
        let entry = self.undo_stack.pop()?;

        // Save current state to redo stack
        self.redo_stack.push(HistoryEntry {
            buffer: current_buffer.snapshot(),
            cursor: current_cursor,
        });

        Some(entry)
    }

    /// Redo: restore next state from redo stack, save current to undo stack
    pub fn redo(
        &mut self,
        current_buffer: &TextBuffer,
        current_cursor: CursorPosition,
    ) -> Option<HistoryEntry> {
        let entry = self.redo_stack.pop()?;

        // Save current state to undo stack
        self.undo_stack.push(HistoryEntry {
            buffer: current_buffer.snapshot(),
            cursor: current_cursor,
        });

        Some(entry)
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
