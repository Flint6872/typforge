use gpui::*;
use gpui_component::{
    IconName,
    dock::Panel as DockPanel,
    h_flex,
    list::ListItem,
    tree::{TreeItem, TreeState, tree},
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
pub struct OpenFileEvent {
    pub path: PathBuf,
}

#[derive(Clone)]
pub struct FilesPanel {
    tree_state: Entity<TreeState>,
    loaded_paths: HashSet<String>,
    pub roots: Vec<TreeItem>,
}

impl FilesPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_project_root = PathBuf::from("crates/typforge/documents");

        eprintln!(
            "FilesPanel: Attempting to use project root: {:?}",
            &default_project_root
        );

        // Ensure the default project root exists, create if not
        if !default_project_root.exists() {
            eprintln!(
                "FilesPanel: Project root does NOT exist. Attempting to create: {:?}",
                &default_project_root
            );
            if let Err(e) = fs::create_dir_all(&default_project_root) {
                eprintln!(
                    "FilesPanel: FAILED to create default project root {:?}: {}",
                    &default_project_root, e
                );
            } else {
                eprintln!(
                    "FilesPanel: Successfully created default project root: {:?}",
                    &default_project_root
                );
            }
        } else {
            eprintln!(
                "FilesPanel: Project root EXISTS: {:?}",
                &default_project_root
            );
        }

        let initial_items = build_file_items(&default_project_root);
        eprintln!(
            "FilesPanel: Number of initial tree items found: {}",
            initial_items.len()
        );

        let initial_items = build_file_items(&default_project_root);

        let tree_state = cx.new(|cx| TreeState::new(cx).items(initial_items.clone()));

        cx.observe(&tree_state, |this, tree_handle, cx| {
            // Get the currently selected entry from the tree
            if let Some(entry) = tree_handle.read(cx).selected_entry() {
                // If it's a folder and it's expanded...
                if entry.is_folder() && entry.is_expanded() {
                    let id = entry.item().id.to_string();
                    // Trigger the lazy load!
                    this.on_item_expanded(id, cx);
                }
            }
        })
        .detach();

        Self {
            tree_state,
            loaded_paths: HashSet::new(),
            roots: initial_items,
        }
    }

    pub fn set_project_root(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.is_dir() {
            let items = build_file_items(&path);

            // FIX: Update our local source of truth
            self.roots = items.clone();
            self.loaded_paths.clear(); // Clear cache for new project

            self.tree_state.update(cx, |state, _cx| {
                state.set_items(items, _cx);
            });
            cx.notify();
        }
    }

    pub fn on_item_expanded(&mut self, item_id: String, cx: &mut Context<Self>) {
        if self.loaded_paths.contains(&item_id) {
            return;
        }

        let path = PathBuf::from(&item_id);
        let item_id_clone = item_id.clone();

        // 1. We use 'move' on the outer closure to capture 'path' and 'item_id_clone'
        cx.spawn(
            move |view: WeakEntity<FilesPanel>, spawned_async_cx: &mut AsyncApp| {
                // 2. Clone the AsyncApp so the async block can own it
                let mut async_app = spawned_async_cx.clone();

                async move {
                    // 3. Perform the blocking I/O (directory scan)
                    let children = build_file_items_sync(&path);

                    // 4. Use the cloned async_app to jump back to the UI thread
                    view.update(&mut async_app, |this, cx| {
                        if update_tree_item_in_vec(&mut this.roots, &item_id_clone, children) {
                            let new_roots = this.roots.clone();

                            // Update the TreeState Entity
                            this.tree_state.update(cx, |state, cx| {
                                state.set_items(new_roots, cx);
                            });

                            this.loaded_paths.insert(item_id_clone);
                            cx.notify();
                        }
                    })
                    .ok();
                }
            },
        )
        .detach();
    }
}

// Helper function to recursively build tree items from a directory
fn build_file_items(path: &Path) -> Vec<TreeItem> {
    eprintln!("build_file_items: Scanning directory: {:?}", path);
    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            eprintln!("build_file_items: Found entry: {:?}", entry_path); // Added this
            let name = entry_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let id = entry_path.to_string_lossy().to_string();

            if entry_path.is_dir() {
                // DON'T recurse here. Just create the node.
                // You can add an empty child or a "Loading..." flag if your TreeItem supports it.
                items.push(TreeItem::new(id, name).expanded(false));
            } else {
                items.push(TreeItem::new(id, name));
            }
        }
    } else {
        eprintln!(
            "build_file_items: FAILED to read directory: {:?} (Error: {:?})",
            path,
            std::io::Error::last_os_error()
        ); // Added this
    }
    items
}

fn build_file_items_sync(path: &Path) -> Vec<TreeItem> {
    let mut items = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            let id = entry_path.to_string_lossy().to_string();

            if entry_path.is_dir() {
                // IMPORTANT: Do NOT recurse here.
                // We just create the directory item. The lazy-loading logic
                // will call this function again for 'id' when expanded.
                items.push(TreeItem::new(id, name).expanded(false));
            } else {
                items.push(TreeItem::new(id, name));
            }
        }
    } else {
        eprintln!("Failed to read: {:?}", path);
    }

    // Optional: Sort items so folders appear first, then alphabetically
    items.sort_by(|a, b| {
        let a_is_dir = Path::new(a.id.as_ref()).is_dir();
        let b_is_dir = Path::new(b.id.as_ref()).is_dir();
        b_is_dir.cmp(&a_is_dir).then(a.label.cmp(&b.label))
    });

    items
}

impl Render for FilesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 1. Capture the Entity handle of this FilesPanel.
        // The Entity handle is 'static and can be moved into the tree closure.
        let view = cx.entity();

        div()
            .size_full()
            //.bg(rgb(0x282828)) // Darker background for file tree
            .child(
                // 2. Use a 'move' closure for the tree to capture 'view'
                // The 5th argument 'app_cx' is the &mut App provided by the tree component.
                tree(
                    &self.tree_state,
                    move |ix, entry, selected, _window, app_cx| {
                        // 3. Use 'view.update' to get a &mut Context<FilesPanel> ('cx' below)
                        // This 'cx' has the correct lifetime and type for this specific item render.
                        view.update(app_cx, |_, cx| {
                            let item = entry.item();
                            let icon = if !entry.is_folder() {
                                IconName::File
                            } else if entry.is_expanded() {
                                IconName::FolderOpen
                            } else {
                                IconName::Folder
                            };

                            let mut list_item = ListItem::new(ix)
                                .selected(selected)
                                .pl(px(16.) * entry.depth() as f32 + px(12.)) // Indent based on depth
                                .child(h_flex().gap_2().child(icon).child(item.label.clone()));

                            if !entry.is_folder() {
                                // Clone the ID and create the path safely
                                let item_id = item.id.clone();
                                let file_path = PathBuf::from(item_id.to_string());

                                // 4. Now cx.listener works perfectly because this 'cx' is correctly lived.
                                list_item = list_item.on_click(cx.listener(
                                    move |_, _event, _win, listener_cx| {
                                        listener_cx.emit(OpenFileEvent {
                                            path: file_path.clone(),
                                        });
                                    },
                                ));
                            }

                            // Return the finished ListItem to the tree component
                            list_item
                        })
                    },
                ),
            )
    }
}

// Implement Focusable for your panel
impl Focusable for FilesPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        cx.focus_handle() // Or create a unique one if needed
    }
}

impl EventEmitter<OpenFileEvent> for FilesPanel {}

// Implement EventEmitter for PanelEvent if you need to emit events
impl EventEmitter<gpui_component::dock::PanelEvent> for FilesPanel {}

impl DockPanel for FilesPanel {
    fn panel_name(&self) -> &'static str {
        "FilesPanel" // Unique string identifier for this panel type
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("FilesPanel")
    }
}

fn update_tree_item_in_vec(
    items: &mut Vec<TreeItem>,
    target_id: &str,
    new_children: Vec<TreeItem>,
) -> bool {
    for item in items.iter_mut() {
        // Check if this is the folder we just loaded
        if item.id == target_id {
            let name = item.label.to_string();

            // Re-create the item with the new children and set to expanded
            *item = TreeItem::new(target_id.to_string(), name)
                .children(new_children)
                .expanded(true);
            return true;
        }

        // If not this item, search inside its existing children (recursive search)
        // Note: This requires access to the children field or method
        if update_tree_item_in_vec(&mut item.children, target_id, new_children.clone()) {
            return true;
        }
    }
    false
}
