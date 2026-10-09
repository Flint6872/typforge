use gpui::*;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{ActiveTheme, StyledExt, h_flex, input::InputState, v_flex};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use super::{PackageIndexItem, PackageManagerStore};

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
    pub expanded_packages: HashSet<String>,
    focus_handle: FocusHandle,
}

impl PackageManagerView {
    pub fn new(
        store: Arc<Mutex<PackageManagerStore>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search packages by name, description, or author...")
        });

        // Re-render when the user types in the search box
        cx.observe(&search_input, |_this, _, cx| {
            cx.notify();
        })
        .detach();

        Self {
            store,
            search_input,
            current_filter: TabFilter::Installed,
            is_syncing: false,
            downloading_packages: HashSet::new(),
            expanded_packages: Hashset::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn toggle_expand(&mut self, name: String, cx: &mut Context<Self>) {
        if self.expanded_packages.contains(&name) {
            self.expanded_packages.remove(&name);
        } else {
            self.expanded_packages.insert(name);
        }
        cx.notify();
    }

    pub fn set_filter(&mut self, filter: TabFilter, cx: &mut Context<Self>) {
        self.current_filter = filter;
        cx.notify();
    }

    /// User-initiated Opt-In Sync (runs network call on GPUI background executor)
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
                        cx.notify();
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
                        cx.notify();
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
        cx.notify();
    }

    pub fn refresh_local(&mut self, cx: &mut Context<Self>) {
        if let Ok(mut store) = self.store.lock() {
            store.scan_local_packages();
        }
        cx.notify();
    }
}

impl Focusable for PackageManagerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PackageManagerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.lock().unwrap();
        let local_packages = store.local_packages.clone();
        let remote_index = store.remote_index.clone();
        let last_error = store.last_error.clone();
        drop(store);

        let query = self.search_input.read(cx).text().to_string().to_lowercase();
        let is_synced = remote_index.is_some();
        let total_registry_count = remote_index.as_ref().map(|v| v.len()).unwrap_or(0);

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
                    // Child 1: Title & Description
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
                    // Child 2: Status Badge & Sync Button (wraps below Child 1 when squeezed)
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
                            .child(self.search_input.clone()),
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
                            .child(format!("Installed ({})", local_packages.len())),
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
            // --- PACKAGE CARD LIST ---
            .child(
                v_flex()
                    .flex_grow()
                    .w_full()
                    .overflow_y_scrollbar()
                    .gap_3()
                    .children(match self.current_filter {
                        TabFilter::Installed => {
                            let filtered = local_packages
                                .iter()
                                .filter(|pkg| {
                                    query.is_empty()
                                        || pkg.name.to_lowercase().contains(&query)
                                        || pkg.namespace.to_lowercase().contains(&query)
                                })
                                .cloned()
                                .collect::<Vec<_>>();

                            if filtered.is_empty() {
                                vec![
                                    div()
                                        .w_full()
                                        .p_12()
                                        .flex()
                                        .justify_center()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("No matching installed packages found on disk.")
                                        .into_any_element(),
                                ]
                            } else {
                                filtered
                                                                  .into_iter()
                                                                  .enumerate()
                                                                  .map(|(ix, pkg)| {
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
                                                                          // Action: Only show "Uninstall" for @preview cache packages
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
                                                                          })
                                                                          .into_any_element()
                                                                  })
                                                                  .collect()
                            }
                        }
                        TabFilter::Registry => {
                            if !is_synced {
                                vec![
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
                                        .into_any_element(),
                                ]
                            } else {
                                let reg_items = remote_index.as_ref().unwrap();

                                let filtered = reg_items
                                    .iter()
                                    .filter(|item| {
                                        if query.is_empty() {
                                            return true;
                                        }
                                        item.name.to_lowercase().contains(&query)
                                            || item
                                                .description
                                                .as_ref()
                                                .map_or(false, |d| d.to_lowercase().contains(&query))
                                            || item
                                                .authors
                                                .iter()
                                                .any(|a| a.to_lowercase().contains(&query))
                                            || item.keywords.as_ref().map_or(false, |kw| {
                                                kw.iter().any(|k| k.to_lowercase().contains(&query))
                                            })
                                    })
                                    .take(80) // Cap layout nodes to top 80 matches for 60 FPS performance
                                    .collect::<Vec<_>>();

                                if filtered.is_empty() {
                                    vec![
                                        div()
                                            .w_full()
                                            .p_12()
                                            .flex()
                                            .justify_center()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("No registry packages match your search.")
                                            .into_any_element(),
                                    ]
                                } else {
                                    filtered
                                        .into_iter()
                                        .enumerate()
                                        .map(|(ix, item)| {
                                            let is_cached = local_packages.iter().any(|lp| {
                                                lp.namespace == "preview"
                                                    && lp.name == item.name
                                                    && lp.version == item.version
                                            });
                                            let is_downloading = self
                                                .downloading_packages
                                                .contains(&(item.name.clone(), item.version.clone()));

                                            render_registry_card(
                                                ix,
                                                item,
                                                is_cached,
                                                is_downloading,
                                                cx,
                                            )
                                        })
                                        .collect()
                                }
                            }
                        }


                    }),
            )
    }
}

fn render_registry_card(
    ix: usize,
    item: &PackageIndexItem,
    is_cached: bool,
    is_downloading: bool,
    cx: &mut Context<PackageManagerView>,
) -> AnyElement {
    let name = item.name.clone();
    let version = item.version.clone();

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
        cx.theme().foreground
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
                // Left Title & Meta
                .child(
                    h_flex()
                        .flex_1()
                        .gap_2()
                        .items_center()
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
                // Right Action Button (flex_shrink_0 ensures it's always visible!)
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
        // Authors & Keywords
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
