use std::path::{Path, PathBuf};

/// Span half-open, medido en bytes UTF-8 de la fuente original.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    #[must_use]
    pub const fn empty() -> Self {
        Self { start: 0, end: 0 }
    }
    #[must_use]
    pub fn start(self) -> usize {
        self.start
    }
    #[must_use]
    pub fn end(self) -> usize {
        self.end
    }
}

/// Ubicación 1-based en líneas y columnas Unicode (scalar values).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// Fuente inmutable con índices de línea y validación de offsets UTF-8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    name: Option<PathBuf>,
    text: String,
    line_checkpoints: Vec<(usize, usize)>,
}

impl SourceFile {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self::build(None, text.into())
    }

    #[must_use]
    pub fn with_path(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self::build(Some(path.into()), text.into())
    }

    fn build(name: Option<PathBuf>, text: String) -> Self {
        let mut line_checkpoints = vec![(1, 0)];
        let mut line = 1;
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line += 1;
                if line % 128 == 1 {
                    line_checkpoints.push((line, index + 1));
                }
            }
        }
        Self {
            name,
            text,
            line_checkpoints,
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.name.as_deref()
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Crea un span únicamente si los offsets son válidos y están en límites UTF-8.
    #[must_use]
    pub fn span(&self, start: usize, end: usize) -> Option<Span> {
        (start <= end
            && end <= self.text.len()
            && self.text.is_char_boundary(start)
            && self.text.is_char_boundary(end))
        .then_some(Span { start, end })
    }

    #[must_use]
    pub fn slice(&self, span: Span) -> &str {
        &self.text[span.start..span.end]
    }

    /// Convierte un offset UTF-8 a línea/columna 1-based.
    #[must_use]
    pub fn position(&self, offset: usize) -> Option<SourcePosition> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return None;
        }
        let checkpoint = self
            .line_checkpoints
            .partition_point(|(_, start)| *start <= offset)
            .saturating_sub(1);
        let (base_line, base_offset) = self.line_checkpoints[checkpoint];
        let prefix = &self.text[base_offset..offset];
        let additional_lines = prefix.bytes().filter(|byte| *byte == b'\n').count();
        let line_start = prefix
            .rfind('\n')
            .map_or(base_offset, |relative| base_offset + relative + 1);
        let column = self.text[line_start..offset].chars().count() + 1;
        Some(SourcePosition {
            line: base_line + additional_lines,
            column,
        })
    }
}
