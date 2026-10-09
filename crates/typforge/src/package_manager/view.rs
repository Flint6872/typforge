use gpui::*;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{
    ActiveTheme, StyledExt, h_flex,
    input::{Input, InputState},
    v_flex,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use super::{LocalPackageInfo, PackageIndexItem, PackageManagerStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabFilter {
    Installed,
    Registry,
}

pub struct PackageManagerView {
    pub store: Arc<Mutex<PackageManagerStore>>,
    pub search_input: Entity<InputState>,
    pub current_filter: TabFilter,
    pub is_syncing: bool,
    pub downloading_packages: HashSet<(String, String)>,
    pub expanded_packages: HashSet<String>, // Set of package names currently expanded

    // Tracking fields to prevent scroll snaps
    pub last_query: String,
    pub last_filter: TabFilter,

    // Virtualized list caching
    pub filtered_installed: Vec<LocalPackageInfo>,
    pub filtered_registry: Vec<PackageIndexItem>,
    pub list_state: ListState,
    focus_handle: FocusHandle,
}

impl PackageManagerView {
    pub fn new(
        store: Arc<Mutex<PackageManagerStore>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search packages by name"));

        // Initialize empty lists & placeholder list state
        let filtered_installed = Vec::new();
        let filtered_registry = Vec::new();
        let list_state = ListState::new(0, ListAlignment::Top, px(40.0));

        let mut this = Self {
            store,
            search_input,
            current_filter: TabFilter::Installed,
            is_syncing: false,
            downloading_packages: HashSet::new(),
            expanded_packages: HashSet::new(),
            last_query: String::new(),
            last_filter: TabFilter::Installed,
            filtered_installed,
            filtered_registry,
            list_state,
            focus_handle: cx.focus_handle(),
        };

        // Recalculate filtered items when search input changes
        cx.observe(&this.search_input, |this_view, _, cx| {
            this_view.update_filtered_items(cx);
        })
        .detach();

        // Perform initial calculation
        this.update_filtered_items(cx);
        this
    }

    /// Recalculates the list of elements matching the filter & search query and updates ListState
    pub fn update_filtered_items(&mut self, cx: &mut Context<Self>) {
        let store = self.store.lock().unwrap();
        let local_packages = store.local_packages.clone();
        let remote_index = store.remote_index.clone();
        drop(store);

        let query = self.search_input.read(cx).text().to_string().to_lowercase();

        let query_changed = query != self.last_query;
        let filter_changed = self.current_filter != self.last_filter;

        self.last_query = query.clone();
        self.last_filter = self.current_filter;

        match self.current_filter {
            TabFilter::Installed => {
                self.filtered_installed = local_packages
                    .into_iter()
                    .filter(|pkg| {
                        query.is_empty()
                            || pkg.name.to_lowercase().starts_with(&query)
                            || pkg.namespace.to_lowercase().starts_with(&query)
                    })
                    .collect();

                if query_changed || filter_changed {
                    self.list_state.reset(self.filtered_installed.len());
                } else {
                    self.list_state.remeasure();
                }
            }
            TabFilter::Registry => {
                if let Some(reg_items) = remote_index {
                    self.filtered_registry = reg_items
                        .into_iter()
                        .filter(|item| {
                            if query.is_empty() {
                                return true;
                            }
                            item.name.to_lowercase().starts_with(&query)
                        })
                        .collect();
                } else {
                    self.filtered_registry = Vec::new();
                }

                if query_changed || filter_changed {
                    self.list_state.reset(self.filtered_registry.len());
                } else {
                    self.list_state.remeasure();
                }
            }
        }
        cx.notify();
    }

    pub fn set_filter(&mut self, filter: TabFilter, cx: &mut Context<Self>) {
        self.current_filter = filter;
        self.update_filtered_items(cx);
    }

    pub fn toggle_expand(&mut self, name: String, cx: &mut Context<Self>) {
        if self.expanded_packages.contains(&name) {
            self.expanded_packages.remove(&name);
        } else {
            self.expanded_packages.insert(name);
        }
        self.update_filtered_items(cx);
    }

    /// User-initiated Opt-In Sync
    pub fn sync_remote_registry(&mut self, cx: &mut Context<Self>) {
        if self.is_syncing {
            return;
        }

        self.is_syncing = true;
        let store = self.store.clone();
        cx.notify();

        cx.spawn(
            move |this: WeakEntity<Self>, spawned_async_cx: &mut AsyncApp| {
                let mut async_cx = spawned_async_cx.clone();

                async move {
                    let result = async_cx
                        .background_executor()
                        .spawn(async move {
                            let mut lock = store.lock().unwrap();
                            lock.fetch_remote_index()
                        })
                        .await;

                    let _ = this.update(&mut async_cx, |this_view, cx| {
                        this_view.is_syncing = false;
                        if result.is_ok() {
                            this_view.current_filter = TabFilter::Registry;
                        }
                        this_view.update_filtered_items(cx);
                    });
                }
            },
        )
        .detach();
    }

    pub fn download_package(&mut self, name: String, version: String, cx: &mut Context<Self>) {
        let key = (name.clone(), version.clone());
        if self.downloading_packages.contains(&key) {
            return;
        }

        self.downloading_packages.insert(key.clone());
        let store = self.store.clone();
        cx.notify();

        cx.spawn(
            move |this: WeakEntity<Self>, spawned_async_cx: &mut AsyncApp| {
                let mut async_cx = spawned_async_cx.clone();
                let (pkg_name, pkg_ver) = key.clone();

                async move {
                    let _result = async_cx
                        .background_executor()
                        .spawn(async move {
                            let mut lock = store.lock().unwrap();
                            lock.download_package(&pkg_name, &pkg_ver)
                        })
                        .await;

                    let _ = this.update(&mut async_cx, |this_view, cx| {
                        this_view.downloading_packages.remove(&key);
                        this_view.update_filtered_items(cx);
                    });
                }
            },
        )
        .detach();
    }

    pub fn delete_package(&mut self, name: String, version: String, cx: &mut Context<Self>) {
        if let Ok(mut store) = self.store.lock() {
            let _ = store.remove_cached_package(&name, &version);
        }
        self.update_filtered_items(cx);
    }

    pub fn refresh_local(&mut self, cx: &mut Context<Self>) {
        if let Ok(mut store) = self.store.lock() {
            store.scan_local_packages();
        }
        self.update_filtered_items(cx);
    }

    /// Clean, virtualized render-handler for list elements
    pub fn render_list_item(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        match self.current_filter {
            TabFilter::Installed => {
                let pkg = &self.filtered_installed[ix];
                let is_local = pkg.namespace == "local";
                let badge_bg: Hsla = if is_local {
                    cx.theme().primary
                } else {
                    cx.theme().accent
                };

                let name = pkg.name.clone();
                let version = pkg.version.clone();
                let danger_fg: Hsla = rgb(0xFF7777).into();

                div()
                    .py_1()
                    .child(
                        div()
                            .w_full()
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().tab_bar)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .font_semibold()
                                                    .text_base()
                                                    .child(pkg.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .text_xs()
                                                    .bg(badge_bg)
                                                    .text_color(cx.theme().background)
                                                    .child(format!("@{}", pkg.namespace)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!("v{}", pkg.version)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(pkg.path.to_string_lossy().to_string()),
                                    ),
                            )
                            .child(if !is_local {
                                div()
                                    .id(("uninstall-btn", ix))
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .font_medium()
                                    .cursor_pointer()
                                    .bg(cx.theme().muted)
                                    .text_color(danger_fg)
                                    .hover(|s| s.opacity(0.8))
                                    .on_click(cx.listener({
                                        let name = name.clone();
                                        let version = version.clone();
                                        move |this, _, _win, cx| {
                                            this.delete_package(name.clone(), version.clone(), cx);
                                        }
                                    }))
                                    .child("Uninstall")
                                    .into_any_element()
                            } else {
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Local Dev")
                                    .into_any_element()
                            }),
                    )
                    .into_any_element()
            }
            TabFilter::Registry => {
                let item = &self.filtered_registry[ix];
                let is_cached = self.filtered_installed.iter().any(|lp| {
                    lp.namespace == "preview" && lp.name == item.name && lp.version == item.version
                });
                let is_downloading = self
                    .downloading_packages
                    .contains(&(item.name.clone(), item.version.clone()));
                let is_expanded = self.expanded_packages.contains(&item.name);

                render_registry_card(ix, item, is_cached, is_downloading, is_expanded, cx)
            }
        }
    }
}

impl Render for PackageManagerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.lock().unwrap();
        let remote_index = store.remote_index.clone();
        let last_error = store.last_error.clone();
        drop(store);

        let is_synced = remote_index.is_some();
        let total_registry_count = remote_index.as_ref().map(|v| v.len()).unwrap_or(0);

        let view_handle = cx.entity().clone();

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .p_6()
            .gap_4()
            // --- TOP HEADER ---
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_2xl().font_bold().child("Typst Universe Packages"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Offline-first manager for @preview and @local Typst packages."),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .text_xs()
                                    .font_medium()
                                    .bg(cx.theme().tab_bar)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .child(if is_synced {
                                        format!("Registry Synced ({} available)", total_registry_count)
                                    } else {
                                        "Local Mode (Offline)".to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .id("sync-btn")
                                    .px_4()
                                    .py_1p5()
                                    .rounded_md()
                                    .text_xs()
                                    .font_bold()
                                    .cursor_pointer()
                                    .bg(if self.is_syncing {
                                        cx.theme().tab_bar
                                    } else {
                                        cx.theme().primary
                                    })
                                    .text_color(if self.is_syncing {
                                        cx.theme().muted_foreground
                                    } else {
                                        cx.theme().background
                                    })
                                    .hover(|s| {
                                        if !self.is_syncing {
                                            s.opacity(0.85)
                                        } else {
                                            s
                                        }
                                    })
                                    .on_click(cx.listener(|this, _, _win, cx| {
                                        this.sync_remote_registry(cx);
                                    }))
                                    .child(if self.is_syncing {
                                        "Syncing Registry..."
                                    } else if is_synced {
                                        "Re-sync Registry"
                                    } else {
                                        "Fetch Remote Registry"
                                    }),
                            ),
                    ),
            )
            // Error banner if any
            .children(last_error.map(|err| {
                div()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x451A1A))
                    .text_color(rgb(0xFF9999))
                    .text_sm()
                    .child(format!("Error: {}", err))
            }))
            // --- SEARCH & FILTER CONTROLS ---
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_grow()
                            .p_1()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().tab_bar)
                            .child(Input::new(&self.search_input)
                                .cleanable(true)),
                    )
                    .child(
                        div()
                            .id("tab-installed")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_xs()
                            .font_semibold()
                            .cursor_pointer()
                            .bg(if self.current_filter == TabFilter::Installed {
                                cx.theme().tab_active
                            } else {
                                cx.theme().tab_bar
                            })
                            .border_1()
                            .border_color(cx.theme().border)
                            .on_click(cx.listener(|this, _, _win, cx| {
                                this.set_filter(TabFilter::Installed, cx);
                            }))
                            .child(format!("Installed ({})", self.filtered_installed.len())),
                    )
                    .child(
                        div()
                            .id("tab-registry")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_xs()
                            .font_semibold()
                            .cursor_pointer()
                            .bg(if self.current_filter == TabFilter::Registry {
                                cx.theme().tab_active
                            } else {
                                cx.theme().tab_bar
                            })
                            .border_1()
                            .border_color(cx.theme().border)
                            .on_click(cx.listener(|this, _, _win, cx| {
                                this.set_filter(TabFilter::Registry, cx);
                            }))
                            .child(format!("All Registry ({})", total_registry_count)),
                    ),
            )
            // --- VIRTUALIZED SCROLL LIST ---
            .child(
                v_flex()
                    .flex_grow()
                    .w_full()
                    .h_full()
                    .child(
                        if self.current_filter == TabFilter::Registry && !is_synced {
                            v_flex()
                                .w_full()
                                .p_12()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .text_base()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Remote registry has not been fetched yet."),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("TypForge runs offline by default. Click 'Fetch Remote Registry' above to load packages from Typst Universe."),
                                )
                                .into_any_element()
                        } else if (self.current_filter == TabFilter::Installed && self.filtered_installed.is_empty())
                            || (self.current_filter == TabFilter::Registry && self.filtered_registry.is_empty())
                        {
                            div()
                                .w_full()
                                .p_12()
                                .flex()
                                .justify_center()
                                .text_color(cx.theme().muted_foreground)
                                .child("No matching packages found.")
                                .into_any_element()
                        } else {
                            // GPUI Native Virtualized List!
                            list(self.list_state.clone(), move |ix, _window, cx| {
                                view_handle.update(cx, |this_view, cx| {
                                    this_view.render_list_item(ix, cx)
                                })
                            })
                            .flex_grow()
                            .w_full()
                            .h_full()
                            .into_any_element()
                        }
                    )
            )
    }
}

fn render_registry_card(
    ix: usize,
    item: &PackageIndexItem,
    is_cached: bool,
    is_downloading: bool,
    is_expanded: bool,
    cx: &mut Context<PackageManagerView>,
) -> AnyElement {
    let name = item.name.clone();
    let version = item.version.clone();
    let danger_fg: Hsla = rgb(0xFF7777).into();

    let btn_bg = if is_downloading {
        cx.theme().muted
    } else if is_cached {
        cx.theme().muted
    } else {
        cx.theme().primary
    };

    let btn_fg = if is_downloading {
        cx.theme().muted_foreground
    } else if is_cached {
        danger_fg
    } else {
        cx.theme().background
    };

    v_flex()
        .w_full()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().tab_bar)
        .gap_2()
        // Header Row
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_4()
                .child(
                    h_flex()
                        .id(("card-header", ix)) // ID assigned here so on_click operates correctly!
                        .flex_1()
                        .gap_2()
                        .items_center()
                        .cursor_pointer()
                        .on_click(cx.listener({
                            let name = name.clone();
                            move |this, _, _win, cx| {
                                this.toggle_expand(name.clone(), cx);
                            }
                        }))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(if is_expanded { "▼" } else { "▶" }),
                        )
                        .child(div().font_bold().text_base().child(item.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("v{}", item.version)),
                        )
                        .children(item.license.as_ref().map(|lic| {
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .text_xs()
                                .bg(cx.theme().muted)
                                .text_color(cx.theme().foreground)
                                .child(lic.clone())
                        })),
                )
                .child(
                    div()
                        .id(("reg-action-btn", ix))
                        .flex_shrink_0()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .font_bold()
                        .cursor_pointer()
                        .bg(btn_bg)
                        .text_color(btn_fg)
                        .hover(|s| if !is_downloading { s.opacity(0.85) } else { s })
                        .on_click(cx.listener({
                            let name = name.clone();
                            let version = version.clone();
                            move |this, _, _win, cx| {
                                if is_downloading {
                                    return;
                                }
                                if is_cached {
                                    this.delete_package(name.clone(), version.clone(), cx);
                                } else {
                                    this.download_package(name.clone(), version.clone(), cx);
                                }
                            }
                        }))
                        .child(if is_downloading {
                            "Downloading..."
                        } else if is_cached {
                            "Uninstall"
                        } else {
                            "Install"
                        }),
                ),
        )
        // Description
        .children(item.description.as_ref().map(|desc| {
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(desc.clone())
        }))
        // Expandable Details Section
        .children(if is_expanded {
            Some(
                v_flex()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Entrypoint:"),
                            )
                            .child(div().text_xs().child(item.entrypoint.clone())),
                    )
                    .children(item.compiler.as_ref().map(|comp| {
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Required Compiler:"),
                            )
                            .child(div().text_xs().child(comp.clone()))
                    }))
                    .children(item.repository.as_ref().map(|repo| {
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Repository:"),
                            )
                            .child(div().text_xs().child(repo.clone()))
                    }))
                    .children(item.categories.as_ref().map(|cats| {
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Categories:"),
                            )
                            .child(div().text_xs().child(cats.join(", ")))
                    })),
            )
        } else {
            None
        })
        // Authors Row
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("by {}", item.authors.join(", "))),
                )
                .children(item.keywords.as_ref().map(|kws| {
                    h_flex().gap_1().children(
                        kws.iter()
                            .take(3)
                            .map(|kw| {
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .text_xs()
                                    .bg(cx.theme().tab_active)
                                    .text_color(cx.theme().muted_foreground)
                                    .child(kw.clone())
                            })
                            .collect::<Vec<_>>(),
                    )
                })),
        )
        .into_any_element()
}
