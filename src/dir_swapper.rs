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
    use core::contracts::build_check_ensures;
    use std::{collections::HashSet, fs, path::Path, sync::LazyLock};

    use tempfile::TempDir;

    use super::*;

    fn new_temp_dir() -> TempDir {
        tempfile::tempdir().expect("failed to create temporary test directory")
    }

    /// Set by create swapper as active name by default.
    const DEFAULT_PRIMARY_NAME: &str = "Example1";
    /// Creates a new swapper with the provided paths or temporary directories, and a primary name
    /// of `DEFAULT_PRIMARY_NAME`.
    fn new_swapper(primary_dir: Option<TempDir>, version_dir: Option<TempDir>) -> DirSwapper {
        let primary_dir = primary_dir.unwrap_or(new_temp_dir());
        let version_dir = version_dir.unwrap_or(new_temp_dir());

        (
            primary_dir,
            version_dir,
            DirSwapper::new(primary_dir.path(), version_dir.path(), DEFAULT_PRIMARY_NAME),
        )
    }
    #[derive(Debug, Clone)]
    enum Node {
        File(PathBuf),
        Dir(PathBuf, Vec<Node>),
    }
    #[derive(Debug, Clone)]
    /// All file paths must be relative to the root.
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
    /// Pre-order itearaton of the file tree.
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

    static DUMMY_FILE_TREE_1: LazyLock<FileTree> = LazyLock::new(|| {
        FileTree(vec![
            Node::File("file1.txt".into()),
            Node::File("file2.txt".into()),
            Node::Dir("inner".into(), vec![Node::File("inner/file3.txt".into())]),
        ])
    });
    static DUMMY_FILE_TREE_2: LazyLock<FileTree> = LazyLock::new(|| {
        FileTree(vec![
            Node::File("Cargo.toml".into()),
            Node::File("Cargo.lock".into()),
            Node::Dir(
                "src".into(),
                vec![
                    Node::File("src/main.rs".into()),
                    Node::File("src/app.rs".into()),
                ],
            ),
        ])
    });

    /// Creates the structure of the specific file tree. Files will be empty.
    fn build_file_tree(dest: impl AsRef<Path>, tree: &FileTree) {
        let dest: PathBuf = dest.as_ref().into();
        for node in tree.into_iter() {
            match node {
                Node::File(path) => fs::write(dest.clone().join(path), "").unwrap(),
                Node::Dir(path, _) => fs::create_dir(dest.clone().join(path)).unwrap(),
            };
        }
    }

    /// Compares if the structure of the directory in `path` match exactly the `expected`. Only checks
    /// file structure, not contents.
    fn is_dir_structure_eq(path: impl AsRef<Path>, expected: &FileTree) -> bool {
        let path: PathBuf = path.as_ref().into();
        let mut file_paths: HashSet<_> = fs::read_dir(&path)
            .unwrap()
            .map(|entry| entry.unwrap().path().canonicalize().unwrap())
            .collect();
        expected.into_iter().all(|node| match node {
            Node::File(file_path) => {
                file_paths.remove(&path.clone().join(file_path).canonicalize().unwrap())
            }
            Node::Dir(dir_path, _) => {
                file_paths.remove(&path.clone().join(dir_path).canonicalize().unwrap())
            }
        }) && file_paths.is_empty()
    }

    // DirSwap Basic Concept: active dir has only active, the versions dir only has inactive versions
    // stored by the name provided. The subfolders in version_dir only store the data and nothing
    // else.

    #[test]
    fn constructor_does_not_modify_primary_dir() {
        let primary_dir = new_temp_dir();
        build_file_tree(primary_dir, &DUMMY_FILE_TREE_1);

        let swapper = new_swapper(Some(primary_dir), None);

        assert!(is_dir_structure_eq(
            swapper.primary_dir(),
            &DUMMY_FILE_TREE_1
        ));
    }

    #[test]
    fn swap_replaces_old_contents_with_new() {
        let primary_dir = new_temp_dir();
        build_file_tree(primary_dir, &DUMMY_FILE_TREE_1);

        let swapper = new_swapper(Some(primary_dir), None);
        let example2_path = swapper.add_version("Example2");
        build_file_tree(example2_path, &DUMMY_FILE_TREE_2);

        swapper.set_active("Example2").unwrap();
        assert!(is_dir_structure_eq(
            swapper.primary_dir(),
            &DUMMY_FILE_TREE_2
        ));
    }

    #[test]
    fn swap_updates_version_identifier() {
        let swapper = new_swapper(None, None);

        swapper.add_version("Example2");

        swapper.set_active("Example2").unwrap();

        assert_eq!(swapper.active_version(), "Example2");
    }

    #[test]
    fn non_existent_name_is_invalid() {
        let swapper = new_swapper(None, None);

        assert!(swapper.set_active("Invalid").is_err())
    }

    #[test]
    fn double_swap_restores_original_dir_contents() {
        let primary_dir = new_temp_dir();
        build_file_tree(primary_dir, &DUMMY_FILE_TREE_1);

        let swapper = new_swapper(Some(primary_dir), None);
        let example2_path = swapper.add_version("Example2");
        build_file_tree(example2_path, &DUMMY_FILE_TREE_2);

        swapper.set_active("Example2").unwrap();
        swapper.set_active(DEFAULT_PRIMARY_NAME);

        assert!(is_dir_structure_eq(
            swapper.primary_dir(),
            &DUMMY_FILE_TREE_1
        ));
    }

    #[test]
    fn double_swap_restores_orginal_version_identifier() {
        let swapper = new_swapper(None, None);

        swapper.add_version("Example2");

        swapper.set_active("Example2").unwrap();
        swapper.set_active(DEFAULT_PRIMARY_NAME);

        assert_eq!(swapper.active_version(), DEFAULT_PRIMARY_NAME);
    }

    #[test]
    fn add_version_creates_an_empty_dir() {
        let swapper = new_swapper(None, None);

        let path = swapper.add_version("Example2");

        assert!(is_dir_structure_eq(path, &FileTree(vec![])));
    }

    #[test]
    fn add_version_uses_existing_dir() {
        let version_dir = new_temp_dir();
        build_file_tree(
            version_dir.path().to_path_buf().join("Example2"),
            &DUMMY_FILE_TREE_1,
        );

        let swapper = new_swapper(None, Some(version_dir));

        let path = swapper.add_version("Example2");

        assert!(is_dir_structure_eq(path, &DUMMY_FILE_TREE_1));
    }

    #[test]
    fn swap_replaces_version_dir_contents() {
        let swapper = new_swapper(None, None);

        let example2_path = swapper.add_version("Example2");
        build_file_tree(example2_path, &DUMMY_FILE_TREE_1);

        swapper.set_active("Example2");

        fs::remove_dir_all(swapper.primary_dir());
        fs::create_dir(swapper.primary_dir());
        build_file_tree(swapper.primary_dir(), &DUMMY_FILE_TREE_2);

        swapper.set_active(DEFAULT_PRIMARY_NAME);

        assert!(is_dir_structure_eq(example2_path, &DUMMY_FILE_TREE_2));
    }

    #[test]
    fn delete_version_removes_version_dir() {
        let swapper = new_swapper(None, None);

        let example2_path = swapper.add_version("Example2");
        build_file_tree(example2_path, &DUMMY_FILE_TREE_1);

        swapper.delete_version("Example2");

        assert!(!fs::exists(example2_path).unwrap());
    }

    // TODO
    #[test]
    fn delete_version_creates_() {
        let swapper = new_swapper(None, None);

        let example2_path = swapper.add_version("Example2");
        build_file_tree(example2_path, &DUMMY_FILE_TREE_1);

        swapper.delete_version("Example2");

        assert!(!fs::exists(example2_path).unwrap());
    }

    // TODO
    #[test]
    fn delete_version_preserves_primary_dir() {
        let swapper = new_swapper(None, None);

        let example2_path = swapper.add_version("Example2");
        build_file_tree(example2_path, &DUMMY_FILE_TREE_1);

        swapper.delete_version("Example2");

        assert!(!fs::exists(example2_path).unwrap());
    }
}
