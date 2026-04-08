use super::*;

use super::*;

    #[test]
    fn insert_and_undo() {
        let mut buf = EditBuffer::new();
        buf.insert_char('h');
        buf.insert_char('i');
        assert_eq!(buf.text(), "hi");
        assert_eq!(buf.cursor(), 2);
        // Consecutive inserts coalesce — one undo reverts both
        buf.undo();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn undo_redo_cycle() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        assert_eq!(buf.text(), "abc");
        buf.undo();
        assert_eq!(buf.text(), "");
        buf.redo();
        assert_eq!(buf.text(), "abc");
    }

    #[test]
    fn backspace_undo() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        buf.backspace();
        assert_eq!(buf.text(), "ab");
        buf.undo();
        assert_eq!(buf.text(), "abc");
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn delete_word_undo() {
        let mut buf = EditBuffer::new();
        for c in "hello world".chars() {
            buf.insert_char(c);
        }
        buf.delete_word_back();
        assert_eq!(buf.text(), "hello ");
        buf.undo();
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn clear_line_undo() {
        let mut buf = EditBuffer::new();
        for c in "test".chars() {
            buf.insert_char(c);
        }
        buf.clear_line();
        assert_eq!(buf.text(), "");
        buf.undo();
        assert_eq!(buf.text(), "test");
    }

    #[test]
    fn redo_cleared_on_new_edit() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.undo();
        assert_eq!(buf.text(), "");
        buf.insert_char('b');
        // Redo should be gone
        assert!(!buf.redo());
        assert_eq!(buf.text(), "b");
    }

    #[test]
    fn non_coalesced_inserts() {
        let mut buf = EditBuffer::new();
        buf.insert_char('a');
        buf.move_home(); // move cursor to 0
        buf.insert_char('b'); // insert at different position — separate undo step
        assert_eq!(buf.text(), "ba");
        buf.undo(); // undo 'b' insert
        assert_eq!(buf.text(), "a");
        buf.undo(); // undo 'a' insert
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn cursor_movement() {
        let mut buf = EditBuffer::new();
        for c in "abc".chars() {
            buf.insert_char(c);
        }
        assert_eq!(buf.cursor(), 3);
        buf.move_left();
        assert_eq!(buf.cursor(), 2);
        buf.move_home();
        assert_eq!(buf.cursor(), 0);
        buf.move_end();
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn word_level_undo() {
        let mut buf = EditBuffer::new();
        for c in "hello world".chars() {
            buf.insert_char(c);
        }
        assert_eq!(buf.text(), "hello world");
        // Undo removes "world" (second word)
        buf.undo();
        assert_eq!(buf.text(), "hello ");
        // Undo removes " " (whitespace group)
        buf.undo();
        assert_eq!(buf.text(), "hello");
        // Undo removes "hello" (first word)
        buf.undo();
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn set_text_clears_stacks() {
        let mut buf = EditBuffer::new();
        for c in "old".chars() {
            buf.insert_char(c);
        }
        buf.set_text("new text");
        assert_eq!(buf.text(), "new text");
        assert_eq!(buf.cursor(), 8);
        // Undo stack was cleared
        assert!(!buf.undo());
    }
