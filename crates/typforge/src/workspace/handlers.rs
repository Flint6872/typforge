use crate::actions;
use crate::editor::editor_panel::EditorPanel; // Import necessary panel types
use crate::workspace::TypstNoteView;
use gpui::*;

impl<W: typst_gpui::TypstGpuiWorld + typforge_core::IdeWorld> TypstNoteView<W> {
    pub(crate) fn handle_file_new(
        &mut self,
        _action: &actions::FileNew,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileNew triggered!");
        self.editor_panel.update(cx, |editor, editor_cx| {
            editor.new_file(window, editor_cx);
        });
    }

    pub(crate) fn handle_file_open(
        &mut self,
        _action: &actions::FileOpen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileOpen triggered!");
        let editor_panel_handle = self.editor_panel.clone();
        let window_handle = window.window_handle();

        cx.spawn(move |_this, spawned_async_cx: &mut AsyncApp| {
            // FIX: Clone the provided `&mut AsyncApp` to get an owned `AsyncApp`
            // that can be moved into the `async move` block.
            let mut cx_for_async_block = spawned_async_cx.clone();

            async move {
                let options = PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("Open File".into()),
                };

                // Request the prompt - returns Receiver<Result<Option<Vec<PathBuf>>, Error>>
                let receiver = cx_for_async_block.update(|app_cx| app_cx.prompt_for_paths(options));

                // Await once and match all possible outcomes
                match receiver.await {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            window_handle
                                .update(&mut cx_for_async_block, |_, window, app_cx| {
                                    editor_panel_handle.update(app_cx, |editor, editor_cx| {
                                        let _ = editor.open_file(path, window, editor_cx);
                                    });
                                })
                                .ok();
                        }
                    }
                    Ok(Ok(None)) => {
                        println!("File selection cancelled by user.");
                    }
                    Ok(Err(e)) => {
                        eprintln!("Error during file selection: {:?}", e);
                    }
                    Err(e) => {
                        eprintln!("Failed to receive paths from prompt: {:?}", e);
                    }
                }
            }
        })
        .detach();
    }

    pub(crate) fn handle_folder_open(
        &mut self,
        _action: &actions::FolderOpen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let files_panel_handle = self.files_panel.clone();
        let window_handle = window.window_handle();

        // Use cx.spawn, but we don't move the original `cx` into the closure.
        // Instead, we rely on the `AsyncApp` provided by `spawn` to perform updates.
        cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
            let mut cx_for_async = spawned_async_cx.clone();

            async move {
                let options = PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Open Directory".into()),
                };

                // Use the cloned AsyncApp to prompt
                let receiver = cx_for_async.update(|app_cx| app_cx.prompt_for_paths(options));

                if let Ok(Ok(Some(paths))) = receiver.await {
                    if let Some(path) = paths.into_iter().next() {
                        let path_str = path.to_string_lossy().to_string();

                        // Update the global state via the AsyncApp clone
                        cx_for_async.update(|app_cx| {
                            let mut settings =
                                app_cx.global::<crate::settings::AppSettings>().clone();
                            settings.last_folder_open = Some(path_str);
                            app_cx.set_global(settings);
                        });

                        // Update the file panel
                        window_handle
                            .update(&mut cx_for_async, |_, _, app_cx| {
                                files_panel_handle.update(app_cx, |files_panel, files_cx| {
                                    files_panel.set_project_root(path, files_cx);
                                });
                            })
                            .ok();
                    }
                }
            }
        })
        .detach();
    }

    pub(crate) fn handle_file_save(
        &mut self,
        _action: &actions::FileSave,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileSave triggered!"); // Debug print
        self.editor_panel
            .update(cx, |editor: &mut EditorPanel<W>, editor_cx| {
                editor.save_active_file(_window, editor_cx);
            });
    }

    pub(crate) fn handle_file_save_as(
        &mut self,
        _action: &actions::FileSaveAs,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileSaveAs triggered!");
        let editor_panel_handle = self.editor_panel.clone();
        let window_handle = window.window_handle();

        cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
            let mut cx_for_async = spawned_async_cx.clone();

            async move {
                let (active_path, dir) = cx_for_async.update(|app_cx| {
                    let editor = editor_panel_handle.read(app_cx);
                    (
                        editor.active_file_path.clone(),
                        Self::get_default_save_dir(editor, app_cx),
                    )
                });

                // If active_path exists, combine the dir and the filename so the dialog
                // defaults to the specific file's location.
                let default_path = if let Some(path) = active_path {
                    dir.join(path.file_name().unwrap_or_default())
                } else {
                    dir // Just the directory if no active file
                };

                let receiver =
                    cx_for_async.update(|app_cx| app_cx.prompt_for_new_path(&default_path, None));

                if let Ok(Ok(Some(path))) = receiver.await {
                    window_handle
                        .update(&mut cx_for_async, |_, window, app_cx| {
                            editor_panel_handle.update(app_cx, |editor, editor_cx| {
                                let _ = editor.save_file_as(path, window, editor_cx);
                            });
                        })
                        .ok();
                }
            }
        })
        .detach();
    }

    pub(crate) fn handle_file_close(
        &mut self,
        _action: &actions::FileClose,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileClose triggered!");
        self.editor_panel.update(cx, |editor, editor_cx| {
            if let Some(path_to_close) = editor.active_file_path.clone() {
                editor.close_file(path_to_close, editor_cx);
            }
        });
    }

    pub(crate) fn handle_file_quit(
        &mut self,
        _action: &actions::FileQuit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileQuit triggered!");
        cx.quit();
    }

    pub(crate) fn handle_export_pdf(
        &mut self,
        _action: &actions::FileExportPdf,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pdf_bytes = self.preview_panel.read(cx).export_pdf();
        let editor = self.editor_panel.read(cx);

        // Get the directory using the helper
        let dir = Self::get_default_save_dir(editor, cx);

        // Determine suggested filename
        let default_name = editor
            .active_file_path
            .as_ref()
            .map(|p| p.with_extension("pdf"))
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .or_else(|| Some("document.pdf".to_string()));

        if let Some(bytes) = pdf_bytes {
            cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
                let cx_for_async_block = spawned_async_cx.clone();

                async move {
                    // 1. Pass the directory path as the first argument
                    // 2. Pass the filename as the second argument (the &str)
                    let receiver = cx_for_async_block
                        .update(|app_cx| app_cx.prompt_for_new_path(&dir, default_name.as_deref()));

                    if let Ok(Ok(Some(path))) = receiver.await {
                        if let Err(e) = std::fs::write(&path, &bytes) {
                            eprintln!("Failed to save export to {:?}: {}", path, e);
                        } else {
                            println!("Successfully exported to {:?}", path);
                        }
                    }
                }
            })
            .detach();
        }
    }

    pub(crate) fn handle_export_docx(
        &mut self,
        _action: &actions::FileExportDocx,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let docx_bytes = self.preview_panel.read(cx).export_docx();
        let editor = self.editor_panel.read(cx);

        // Use the centralized helper for the base directory
        let dir = Self::get_default_save_dir(editor, cx);
        //let dir_clone = dir.clone();

        // Determine suggested filename
        let default_name = editor
            .active_file_path
            .as_ref()
            .map(|p| p.with_extension("docx"))
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .or_else(|| Some("document.docx".to_string()));

        if let Some(bytes) = docx_bytes {
            cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
                let cx_for_async_block = spawned_async_cx.clone();

                async move {
                    // 1. Pass the directory path as the first argument
                    // 2. Pass the filename as the second argument (the &str)
                    let receiver = cx_for_async_block
                        .update(|app_cx| app_cx.prompt_for_new_path(&dir, default_name.as_deref()));

                    if let Ok(Ok(Some(path))) = receiver.await {
                        if let Err(e) = std::fs::write(&path, &bytes) {
                            eprintln!("Failed to save export to {:?}: {}", path, e);
                        } else {
                            println!("Successfully exported to {:?}", path);
                        }
                    }
                }
            })
            .detach();
        } else {
            eprintln!("Export failed: Word conversion returned no data.");
        }
    }

    pub(crate) fn handle_reload_settings(
        &mut self,
        _action: &actions::ReloadSettings,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::settings::load_settings(cx);
        crate::components::theme::apply_settings_theme(cx);
        cx.notify(); // Or cx.notify() to redraw everything
    }

    fn get_default_save_dir(editor: &EditorPanel<W>, cx: &impl AppContext) -> std::path::PathBuf {
        // 1. If we have an active file path, suggest its parent directory
        if let Some(path) = &editor.active_file_path {
            if let Some(parent) = path.parent() {
                return parent.to_path_buf();
            }
        }

        // 2. Fallback to the user's last opened folder
        // We use read_global to access the AppSettings securely
        let last_folder = cx.read_global::<crate::settings::AppSettings, _>(|settings, _| {
            settings.last_folder_open.clone()
        });

        if let Some(folder) = last_folder {
            return std::path::PathBuf::from(folder);
        }

        // 3. Absolute fallback
        std::path::PathBuf::from(".")
    }

    // fn handle_undo(&mut self, _: &actions::EditUndo, window: &mut Window, cx: &mut Context<Self>) {
    //     self.editor_panel
    //         .update(cx, |editor: &mut EditorPanel, editor_cx| {
    //             window.dispatch_action(Box::new(gpui::OsAction::Undo), cx);
    //         });
    // }

    // fn handle_redo(&mut self, _: &actions::EditRedo, window: &mut Window, cx: &mut Context<Self>) {
    //     self.editor_panel
    //         .update(cx, |editor: &mut EditorPanel, editor_cx| {
    //             window.dispatch_action(Box::new(gpui::actions::Redo), cx);
    //         });
    // }
    //
    //
    //
}
