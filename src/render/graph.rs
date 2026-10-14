/// Builds a linear `-filter_complex` graph where each stage reads the previous stage's output.
pub struct FilterGraph {
    stages: Vec<String>,
    current: String,
}

impl FilterGraph {
    pub fn new(input: &str) -> Self {
        Self {
            stages: Vec::new(),
            current: input.to_owned(),
        }
    }

    /// Adds a stage written by `build`, which receives the label to read from and returns the
    /// stage text without its output label.
    pub fn stage(&mut self, build: impl FnOnce(&str) -> String) {
        let output = format!("s{}", self.stages.len() + 1);
        let text = build(&self.current);
        self.stages.push(format!("{text}[{output}]"));
        self.current = output;
    }

    /// Adds a plain comma separated filter chain.
    pub fn chain(&mut self, filters: &str) {
        self.stage(|input| format!("[{input}]{filters}"));
    }

    /// Ends the graph with `filters` writing to the `[outv]` label.
    pub fn finish(mut self, filters: &str) -> String {
        let text = format!("[{}]{filters}[outv]", self.current);
        self.stages.push(text);
        self.stages.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_are_linked_in_order() {
        let mut graph = FilterGraph::new("0:v");
        graph.chain("scale=10:10");
        graph.stage(|input| format!("[{input}]split[a][b];[a][b]hstack"));
        assert_eq!(
            graph.finish("null"),
            "[0:v]scale=10:10[s1];[s1]split[a][b];[a][b]hstack[s2];[s2]null[outv]"
        );
    }
}
