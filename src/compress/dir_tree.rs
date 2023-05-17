use crate::{compress::data_source::ResolvedDataSource, errors::*};
use jwalk::WalkDirGeneric;
use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct DirNode {
    pub(crate) data: DirNodeData,
}
#[derive(Debug)]
pub(crate) enum DirNodeData {
    FileNode { contents: ResolvedDataSource },
    DirNode { contents: HashMap<String, DirNode> },
}
impl DirNode {
    // TODO: Create files from data sources.

    /// Creates a new empty directory.
    pub fn empty_dir() -> DirNode {
        DirNode { data: DirNodeData::DirNode { contents: Default::default() } }
    }

    pub fn add_node(&mut self, name: &str, node: DirNode) {
        if let DirNodeData::DirNode { contents, .. } = &mut self.data {
            contents.insert(name.to_string(), node);
        } else {
            panic!("Attempted add_node on non-directory node.");
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<DirNode> {
        let path = std::fs::canonicalize(path.as_ref())?;
        let path = path.as_path();
        trace!("Building directory tree for {}...", path.display());

        // Find all directories and files in the paths.
        let data: jwalk::Result<Vec<_>> = WalkDirGeneric::<(PathBuf, PathBuf)>::new(path)
            .follow_links(false)
            .skip_hidden(false)
            .sort(true)
            .root_read_dir_state(PathBuf::new())
            .process_read_dir(|_depth, _path, read_dir_state, children| {
                for child in children {
                    if let Ok(child) = child {
                        let mut new_path = read_dir_state.clone();
                        new_path.push(&child.file_name);
                        child.client_state = new_path;
                    }
                }
            })
            .into_iter()
            .collect();
        let data = data?;

        // Convert the linear directory data into the tree model.
        #[derive(Debug)]
        struct DirStack(Vec<(String, DirNode)>);
        impl DirStack {
            fn enter_dir(&mut self, name: String) {
                self.0.push((name.to_string(), DirNode::empty_dir()));
            }
            fn push_file(&mut self, name: String, node: DirNode) {
                match self.0.last_mut() {
                    Some(x) => x.1.add_node(&name, node),
                    None => self.0.push((name.to_string(), node)),
                }
            }
            fn pop_node(&mut self) {
                let (name, node) = self.0.pop().expect("pop_node on root node");
                self.push_file(name, node);
            }
        }

        let mut dirs_stack = DirStack(Vec::new());
        for t in data {
            let mut path = t.parent_path.to_path_buf();
            path.push(t.file_name);

            if t.depth < dirs_stack.0.len() {
                dirs_stack.pop_node();
            }

            let name = path.file_name().unwrap().to_str().unwrap().into(); // TODO: unwraps
            if path.is_dir() {
                dirs_stack.enter_dir(name);
            } else if path.is_file() {
                dirs_stack.push_file(name, DirNode {
                    data: DirNodeData::FileNode {
                        contents: ResolvedDataSource::from_path(&path)?,
                    },
                });
            } else {
                warn!("Path {} is of unknown type!", path.display());
            }
        }
        while dirs_stack.0.len() > 1 {
            dirs_stack.pop_node()
        }

        Ok(dirs_stack.0.pop().expect("Dir stack is empty?").1)
    }
}
