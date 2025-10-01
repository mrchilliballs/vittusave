use anyhow::{Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::utils;

#[derive(Debug, Default)]
struct DirSwapper {
    primary_dir: PathBuf,
    version_dir: PathBuf,
    // Should always be set unless there are no versions
    active_version: Option<String>,
}

impl DirSwapper {
    /// Creates a new `DirSwapper`. The current contents of the primary directory will
    /// automatically get a version and corresponding directory assigned to it named `name`.
    /// Existing contents in the versions directory will be preserved.
    pub fn build(primary_dir: PathBuf, version_dir: PathBuf, name: String) -> Result<Self> {
        let swapper = Self {
            primary_dir,
            version_dir,
            active_version: Some(name),
        };
        if !fs::exists(swapper.build_version_dir(swapper.active_version.as_ref().unwrap()))? {
            fs::create_dir(swapper.build_version_dir(swapper.active_version.as_ref().unwrap()))?;
        }
        Ok(swapper)
    }
    /// If a version is active, it is stored here.
    pub fn primary_dir(&self) -> &Path {
        &self.primary_dir
    }
    /// Stores all the directories containing a version's contents.
    pub fn version_dir(&self) -> &Path {
        &self.version_dir
    }
    /// Builds the
    fn build_version_dir(&self, name: &str) -> PathBuf {
        self.version_dir.join(name)
    }
    pub fn get_version_dir(&self, name: &str) -> Result<Option<PathBuf>> {
        let version_dir = self.build_version_dir(name);
        Ok(fs::exists(&version_dir)?.then_some(version_dir))
    }
    /// Saves the contents of the primary directory to its correct location and replaces it with
    /// the contents inside of version `name`'s directory. Returns an error if version does not
    /// exist.
    pub fn set_active(&mut self, name: String) -> Result<()> {
        if self.get_version_dir(&name)?.is_none() {
            bail!("failed to set active version to \"{name}\", it does not exist");
        }

        let old_version_dir = self
            .get_version_dir(
                self.active_version
                    .as_deref()
                    .expect("active version should be set if any other version exists"),
            )?
            .expect("active version directory should exist");
        let new_version_dir = self
            .get_version_dir(&name)?
            .expect(&format!("version \"{name}\" should exist"));

        utils::remove_dir_contents(&old_version_dir)?;
        utils::copy_dir_all(self.primary_dir(), &old_version_dir)?;
        utils::remove_dir_contents(self.primary_dir())?;
        utils::copy_dir_all(&new_version_dir, self.primary_dir())?;

        self.active_version = Some(name.to_string());
        Ok(())
    }
    /// Add a new version and create a correponding directory. Returns an error if version already
    /// exists.
    pub fn add_version(&mut self, name: &str) -> Result<()> {
        if fs::exists(self.build_version_dir(name))? {
            bail!("failed to add version \"{name}\", it already exists");
        }
        fs::create_dir(self.build_version_dir(name))?;
        Ok(())
    }
    /// Delete a version and its corresponding directory. Returns an error if it does not exist.
    pub fn delete_version(&mut self, name: &str) -> Result<()> {
        if !fs::exists(self.build_version_dir(name))? {
            bail!("failed to delete version \"{name}\", it does not exist");
        }
        if self
            .active_version
            .as_ref()
            .is_some_and(|version_name| version_name == name)
        {
            self.active_version = None;
        }
        fs::remove_dir_all(
            self.get_version_dir(name)?
                .expect("version directory should exist"),
        )?;

        Ok(())
    }
    /// Returns version that is loaded in the primary directory, if any.
    pub fn active_version(&self) -> Option<&str> {
        self.active_version.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fs::{self, DirEntry, ReadDir},
        hash::Hash,
        io,
        path::Path,
        sync::LazyLock,
    };

    use tempfile::TempDir;

    use super::*;

    fn new_temp_dir() -> TempDir {
        tempfile::tempdir().expect("failed to create temporary test directory")
    }

    /// Set by create swapper as active name by default.
    const DEFAULT_NAME: &str = "Example1";
    /// Creates a new swapper with the provided paths or temporary directories, and a primary name
    /// of `DEFAULT_PRIMARY_NAME`.
    // FIXME: return `TempDir`s to avoid leaking files
    fn new_swapper(primary_dir: Option<TempDir>, version_dir: Option<TempDir>) -> DirSwapper {
        let primary_dir = primary_dir.unwrap_or(new_temp_dir());
        let version_dir = version_dir.unwrap_or(new_temp_dir());

        DirSwapper::build(
            primary_dir.keep(),
            version_dir.keep(),
            DEFAULT_NAME.to_string(),
        )
        .unwrap()
    }
    // FIXME: impl hash that only uses file name
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Node {
        File(PathBuf),
        Dir(PathBuf, HashSet<Node>),
    }
    impl Hash for Node {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            match self {
                Node::File(name) => name.hash(state),
                Node::Dir(name, _) => name.hash(state),
            }
        }
    }
    #[derive(Default, Debug, Clone, PartialEq, Eq)]
    /// All file paths must be relative to the root.
    struct FileTree(HashSet<Node>);

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
    impl<'a> From<ReadDir> for FileTree {
        fn from(value: ReadDir) -> Self {
            Self::from_iter(value)
        }
    }
    impl FileTree {
        /// Panics on any error
        fn from_path(path: impl AsRef<Path>) -> Self {
            Self::from(fs::read_dir(path).unwrap())
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
                    let mut iter = children.iter();
                    self.curr = iter.next();
                    for child in iter {
                        self.stack.push(child);
                    }
                    self.curr
                }
                None if self.at_head => {
                    self.at_head = false;
                    let mut iter = self.node_tree.0.iter();
                    self.curr = iter.next();
                    for node in iter {
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
    fn dir_entries_to_file_tree(
        entries: impl IntoIterator<Item = io::Result<DirEntry>>,
        path: Option<PathBuf>,
    ) -> HashSet<Node> {
        let mut results = HashSet::new();
        let path = path.unwrap_or_default();
        for entry in entries {
            let file_type = entry.as_ref().unwrap().file_type().unwrap();
            if file_type.is_file() {
                results.insert(Node::File(path.join(entry.unwrap().file_name())));
            } else if file_type.is_dir() {
                results.insert(Node::Dir(
                    entry.as_ref().unwrap().file_name().into(),
                    dir_entries_to_file_tree(
                        fs::read_dir(entry.as_ref().unwrap().path()).unwrap(),
                        Some(path.join(entry.unwrap().file_name())),
                    ),
                ));
            }
        }
        results
    }

    impl FromIterator<io::Result<DirEntry>> for FileTree {
        /// Panics on errors (unwraps) and skips symlinks
        fn from_iter<T: IntoIterator<Item = io::Result<DirEntry>>>(iter: T) -> Self {
            FileTree(dir_entries_to_file_tree(iter, None))
        }
    }

    static DUMMY_FILE_TREE_1: LazyLock<FileTree> = LazyLock::new(|| {
        FileTree(HashSet::from([
            Node::File("file1.txt".into()),
            Node::File("file2.txt".into()),
            Node::Dir(
                "inner".into(),
                HashSet::from([Node::File("inner/file3.txt".into())]),
            ),
        ]))
    });
    static DUMMY_FILE_TREE_2: LazyLock<FileTree> = LazyLock::new(|| {
        FileTree(HashSet::from([
            Node::File("Cargo.toml".into()),
            Node::File("Cargo.lock".into()),
            Node::Dir(
                "src".into(),
                HashSet::from([
                    Node::File("src/main.rs".into()),
                    Node::File("src/app.rs".into()),
                ]),
            ),
        ]))
    });

    /// Creates the structure of the specific file tree. Files will be empty.
    fn build_file_tree(dest: impl AsRef<Path>, tree: &FileTree) {
        let dest: PathBuf = dest.as_ref().into();
        println!("{:?}", tree);
        for node in tree.into_iter() {
            match node {
                Node::File(path) => {
                    let path = dest.clone().join(path);
                    fs::write(&path, "").unwrap()
                }
                Node::Dir(path, _) => fs::create_dir_all(dest.clone().join(path)).unwrap(),
            };
        }
    }

    #[test]
    fn constructor_does_not_modify_primary_dir() {
        let primary_dir = new_temp_dir();
        build_file_tree(&primary_dir, &DUMMY_FILE_TREE_1);

        let swapper = new_swapper(Some(primary_dir), None);

        println!("PATH: {}", swapper.primary_dir().display());
        assert_eq!(
            FileTree::from_path(swapper.primary_dir()),
            *DUMMY_FILE_TREE_1,
        );
    }

    #[test]
    fn constructor_uses_existing_version_dirs() {
        let temp_dir = new_temp_dir();
        let version_dir1 = temp_dir.path().to_path_buf().join("Example2");
        fs::create_dir(&version_dir1).unwrap();
        build_file_tree(&version_dir1, &DUMMY_FILE_TREE_1);
        let version_dir2 = temp_dir.path().to_path_buf().join("Example3");
        fs::create_dir(&version_dir2).unwrap();
        build_file_tree(&version_dir2, &DUMMY_FILE_TREE_1);

        let swapper = new_swapper(None, Some(temp_dir));

        assert_eq!(
            FileTree::from_path(swapper.get_version_dir("Example2").unwrap().unwrap()),
            *DUMMY_FILE_TREE_1,
        );
        assert_eq!(
            FileTree::from_path(swapper.get_version_dir("Example3").unwrap().unwrap()),
            *DUMMY_FILE_TREE_1,
        );
    }

    #[test]
    fn swap_replaces_old_contents_with_new() {
        let primary_dir = new_temp_dir();
        build_file_tree(&primary_dir, &DUMMY_FILE_TREE_1);

        let mut swapper = new_swapper(Some(primary_dir), None);
        swapper.add_version("Example2").unwrap();
        build_file_tree(
            swapper.get_version_dir("Example2").unwrap().unwrap(),
            &DUMMY_FILE_TREE_2,
        );

        swapper.set_active("Example2".to_string()).unwrap();
        assert_eq!(
            FileTree::from_path(swapper.primary_dir()),
            *DUMMY_FILE_TREE_2
        );
    }

    #[test]
    fn swap_updates_version_identifier() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();

        swapper.set_active("Example2".to_string()).unwrap();

        assert_eq!(swapper.active_version(), Some("Example2"));
    }

    #[test]
    fn non_existent_name_is_invalid() {
        let mut swapper = new_swapper(None, None);

        assert!(swapper.set_active("Invalid".to_string()).is_err())
    }

    #[test]
    fn double_swap_restores_original_dir_contents() {
        let primary_dir = new_temp_dir();
        build_file_tree(&primary_dir, &DUMMY_FILE_TREE_1);

        let mut swapper = new_swapper(Some(primary_dir), None);
        swapper.add_version("Example2").unwrap();
        build_file_tree(
            swapper.get_version_dir("Example2").unwrap().unwrap(),
            &DUMMY_FILE_TREE_2,
        );

        swapper.set_active("Example2".to_string()).unwrap();
        swapper.set_active(DEFAULT_NAME.to_string()).unwrap();

        assert_eq!(
            FileTree::from_path(swapper.primary_dir()),
            *DUMMY_FILE_TREE_1
        );
    }

    #[test]
    fn double_swap_restores_orginal_version_identifier() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();

        swapper.set_active("Example2".to_string()).unwrap();
        swapper.set_active(DEFAULT_NAME.to_string()).unwrap();

        assert_eq!(swapper.active_version(), Some(DEFAULT_NAME));
    }

    #[test]
    fn add_version_creates_an_empty_dir() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();

        assert_eq!(
            FileTree::from_path(swapper.get_version_dir("Example2").unwrap().unwrap()),
            FileTree::default(),
        );
    }

    #[test]
    fn swap_replaces_version_dir_contents() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();
        build_file_tree(
            swapper.get_version_dir("Example2").unwrap().unwrap(),
            &DUMMY_FILE_TREE_1,
        );

        swapper.set_active("Example2".to_string()).unwrap();

        fs::remove_dir_all(swapper.primary_dir()).unwrap();
        fs::create_dir(swapper.primary_dir()).unwrap();
        build_file_tree(swapper.primary_dir(), &DUMMY_FILE_TREE_2);

        swapper.set_active(DEFAULT_NAME.to_string()).unwrap();

        assert_eq!(
            FileTree::from_path(swapper.get_version_dir("Example2").unwrap().unwrap()),
            *DUMMY_FILE_TREE_2
        );
    }

    #[test]
    fn delete_version_removes_version_dir() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();
        build_file_tree(
            swapper.get_version_dir("Example2").unwrap().unwrap(),
            &DUMMY_FILE_TREE_1,
        );

        let version_dir = swapper.get_version_dir("Example2").unwrap().unwrap();
        swapper.delete_version("Example2").unwrap();

        assert!(!fs::exists(version_dir).unwrap());
    }

    #[test]
    fn deleted_version_is_deactivated() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();
        build_file_tree(
            swapper.get_version_dir("Example2").unwrap().unwrap(),
            &DUMMY_FILE_TREE_1,
        );

        swapper.set_active("Example2".to_string()).unwrap();
        swapper.delete_version("Example2").unwrap();

        assert!(swapper.active_version().is_none());
    }

    #[test]
    fn delete_version_does_not_delete_primary_dir() {
        let mut swapper = new_swapper(None, None);

        swapper.add_version("Example2").unwrap();
        build_file_tree(
            swapper.get_version_dir("Example2").unwrap().unwrap(),
            &DUMMY_FILE_TREE_1,
        );

        swapper.delete_version("Example2").unwrap();

        assert!(fs::exists(swapper.primary_dir()).unwrap());
    }
}
