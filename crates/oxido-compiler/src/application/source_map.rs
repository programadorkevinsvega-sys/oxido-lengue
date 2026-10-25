use oxido_rust_backend::SpanMapping;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMap {
    pub version: u32,
    pub source_file: Option<String>,
    pub generated_file: Option<String>,
    pub mappings: Vec<SpanMapping>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
    pub start: usize,
    pub end: usize,
}

impl SourceMap {
    #[must_use]
    pub fn new(mappings: Vec<SpanMapping>) -> Self {
        Self {
            version: 1,
            source_file: None,
            generated_file: None,
            mappings,
        }
    }

    pub fn set_source_file(&mut self, path: Option<&std::path::Path>) {
        self.source_file = path
            .and_then(std::path::Path::file_name)
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_owned);
    }

    pub fn set_generated_file(&mut self, path: Option<&std::path::Path>) {
        self.generated_file = path
            .and_then(std::path::Path::file_name)
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_owned);
    }

    /// Resuelve una columna generada 1-based al fragmento fuente más específico.
    #[must_use]
    pub fn map_generated(&self, line: usize, column: usize) -> Option<SourceLocation> {
        let column = column.checked_sub(1)?;
        let mapping = self
            .mappings
            .iter()
            .filter(|mapping| {
                let starts_before = (line, column)
                    >= (mapping.generated_start_line, mapping.generated_start_column);
                let ends_after =
                    (line, column) < (mapping.generated_end_line, mapping.generated_end_column);
                starts_before && ends_after
            })
            .min_by_key(|mapping| mapping.source_end.saturating_sub(mapping.source_start))?;
        Some(SourceLocation {
            line: mapping.source_start_line,
            column: mapping.source_start_column,
            start: mapping.source_start,
            end: mapping.source_end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_generated_positions_to_the_most_specific_source_fragment() {
        let map = SourceMap::new(vec![
            SpanMapping {
                source_start: 10,
                source_end: 100,
                source_start_line: 1,
                source_start_column: 1,
                source_end_line: 5,
                source_end_column: 1,
                generated_start_line: 1,
                generated_start_column: 0,
                generated_end_line: 5,
                generated_end_column: 1,
            },
            SpanMapping {
                source_start: 50,
                source_end: 60,
                source_start_line: 3,
                source_start_column: 8,
                source_end_line: 3,
                source_end_column: 18,
                generated_start_line: 3,
                generated_start_column: 8,
                generated_end_line: 3,
                generated_end_column: 18,
            },
        ]);
        let mapped = map.map_generated(3, 10).unwrap();
        assert_eq!(
            (mapped.line, mapped.column, mapped.start, mapped.end),
            (3, 8, 50, 60)
        );
        assert_eq!(map.map_generated(3, 30).unwrap().start, 10);
    }
}
