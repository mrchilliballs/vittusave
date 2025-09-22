use std::path::PathBuf;

// #[derive(Debug, Default)]
//     root: Box<dyn Wr,
//     sources: Vec<PathBuf>,
//     active: Option<usize>,
// }
//
// impl DirSwapper {
//     pub fn new(sources: Vec<PathBuf>) -> Self {
//         Self { sources,  ..Default::default() }
//     }
//     pub fn select(&mut self, index: usize) {
//         todo!()
//     }
//     pub fn selected(&mut self) -> &Path {
//         todo!()
//     }
//     pub fn select_next(&mut self) {
//         todo!()
//     }
//     pub fn select_previous(&mut self) {
//         todo!()
//     }
//     pub fn select_first(&mut self) {
//         todo!()
//     }
//     pub fn select_last(&mut self) {
//         todo!()
//     }
// }
//
#[cfg(test)]
mod tests {
    use std::{fs, path::Path, sync::LazyLock};

    use tempfile::TempDir;

    use super::*;

    // fn create_swapper() -> DirSwapper {
    //     let primary_dir = tempfile::tempdir().expect("failed to create temporary test directory");
    //     let version_dir = tempfile::tempdir().expect("failed to create temporary test directory");
    //
    //     DirSwapper::new(primary_dir, versions_dir);
    // }
    #[derive(Debug, Clone)]
    enum Node {
        File(PathBuf),
        Dir(PathBuf, Vec<Node>),
    }
    #[derive(Debug, Clone)]
    /// All file paths must be relative to the root
    struct FileTree(Vec<Node>);

    impl<'a> IntoIterator for &'a FileTree {
        type Item = &'a Node;
        type IntoIter = FileTreeIter<'a>;

        fn into_iter(self) -> Self::IntoIter {
            FileTreeIter {
                node_tree: self,
                curr: None,
                stack: Vec::new(),
                at_head: true,
            }
        }
    }
    #[derive(Debug, Clone)]
    struct FileTreeIter<'a> {
        node_tree: &'a FileTree,
        curr: Option<&'a Node>,
        stack: Vec<&'a Node>,
        at_head: bool,
    }
    /// Pre-order itearaton of the file tree
    impl<'a> Iterator for FileTreeIter<'a> {
        type Item = &'a Node;

        fn next(&mut self) -> Option<Self::Item> {
            match self.curr {
                Some(Node::Dir(_, children)) => {
                    self.curr = children.get(0);
                    for child in children
                        .split_at_checked(1)
                        // TODO: Replace with map_or_default when it stablizes
                        .map_or_else(|| -> &[Node] { Default::default() }, |(_, right)| right)
                        .iter()
                        .rev()
                    {
                        self.stack.push(child);
                    }
                    self.curr
                }
                None if self.at_head => {
                    self.at_head = false;
                    self.curr = self.node_tree.0.get(0);
                    for node in self
                        .node_tree
                        .0
                        .split_at_checked(1)
                        // TODO: Replace with map_or_default when it stablizes
                        .map_or_else(|| -> &[Node] { Default::default() }, |(_, right)| right)
                        .iter()
                        .rev()
                    {
                        self.stack.push(node);
                    }
                    self.curr
                }
                Some(Node::File(_)) | None => {
                    self.curr = self.stack.pop();
                    self.curr
                }
            }
        }
    }

    static FILE_TREE: LazyLock<FileTree> = LazyLock::new(|| {
        FileTree(vec![
            Node::File("file1.txt".into()),
            Node::File("file2.txt".into()),
            Node::Dir("inner".into(), vec![Node::File("inner/file3.txt".into())]),
        ])
    });

    fn fill_with_dummy_contents(dest: impl AsRef<Path>) {
        let dest: PathBuf = dest.as_ref().into();
        for node in FILE_TREE.into_iter() {
            match node {
                Node::File(path) => fs::write(dest.clone().join(path), "").unwrap(),
                Node::Dir(path, _) => fs::create_dir(dest.clone().join(path)).unwrap(),
            };
        }
    }

    fn check_contents(src: impl AsRef<Path>, expected: &FileTree) -> bool {
        let src: PathBuf = src.as_ref().into();
        expected.into_iter().all(|node| match node {
            Node::File(path) => fs::exists(src.clone().join(path)).unwrap(),
            Node::Dir(path, _) => fs::exists(src.clone().join(path)).unwrap(),
        })
    }

    // #[test]
    // fn old_contents_are_saved_after_swap() {
    //     let swapper = create_swapper();
    //
    //     swapper.new_version();
    // }
    //
    // #[test]
    // fn new_contents_become_active_after_swap() {}
}
