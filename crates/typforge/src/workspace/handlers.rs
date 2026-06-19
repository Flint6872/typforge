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
                            settings.default_save_folder = Some(path_str);
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

        // 1. Capture the current path context
        let active_file_path = editor_panel_handle.read(cx).active_file_path.clone();

        // 2. Capture the default_save_folder from settings before spawning
        let default_save_folder = cx
            .global::<crate::settings::AppSettings>()
            .default_save_folder
            .as_ref()
            .map(std::path::PathBuf::from);

        cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
            let mut cx_for_async_block = spawned_async_cx.clone();

            async move {
                // 3. Determine the directory:
                // Priority: Active File Folder > Settings Default Folder > Current Directory
                let dir = if let Some(ref p) = active_file_path {
                    p.parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                } else if let Some(ref d) = default_save_folder {
                    d.clone()
                } else {
                    std::path::PathBuf::from(".")
                };

                let name = active_file_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str());

                let receiver =
                    cx_for_async_block.update(|app_cx| app_cx.prompt_for_new_path(&dir, name));

                if let Ok(Ok(Some(path))) = receiver.await {
                    window_handle
                        .update(&mut cx_for_async_block, |_, window, app_cx| {
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
        println!("Action: FileExportPdf triggered!");

        // 1. Get the bytes from the preview panel
        let pdf_bytes = self.preview_panel.read(cx).export_pdf();

        // 2. Determine default filename and directory from the active file in the editor
        let editor = self.editor_panel.read(cx);
        let (default_dir, default_name) = if let Some(path) = &editor.active_file_path {
            // Get the directory containing the file
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));

            // Generate "filename.pdf" from "filename.typ"
            let name = path
                .with_extension("pdf")
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "document.pdf".to_string());

            (dir, Some(name))
        } else {
            // Default fallback for unsaved files
            (
                std::path::PathBuf::from("."),
                Some("document.pdf".to_string()),
            )
        };

        if let Some(bytes) = pdf_bytes {
            cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
                let cx_for_async_block = spawned_async_cx.clone();
                let bytes = bytes;
                let dir = default_dir;
                let name = default_name;

                async move {
                    let name_ref = name.as_deref();
                    let receiver = cx_for_async_block
                        .update(|app_cx| app_cx.prompt_for_new_path(&dir, name_ref));

                    match receiver.await {
                        Ok(Ok(Some(path))) => {
                            // Success: User picked a path
                            match std::fs::write(&path, &bytes) {
                                Ok(_) => println!("Successfully exported PDF to {:?}", path),
                                Err(e) => eprintln!("Failed to save PDF to {:?}: {}", path, e),
                            }
                        }
                        Ok(Ok(None)) => {
                            // Success: User cancelled the dialog
                            println!("Export cancelled by user.");
                        }
                        Ok(Err(e)) => {
                            // The prompt logic itself returned an error
                            eprintln!("Error during prompt interaction: {:?}", e);
                        }
                        Err(_) => {
                            // The oneshot channel was cancelled (e.g. sender dropped)
                            eprintln!("Prompt channel was closed unexpectedly.");
                        }
                    }
                }
            })
            .detach();
        } else {
            eprintln!("Export failed: No compiled document available.");
            // To handle the alert popover mentioned earlier, you could update a state field here
            // self.export_error = Some("No compiled document".into());
            // cx.notify();
        }
    }

    pub(crate) fn handle_export_docx(
        &mut self,
        _action: &actions::FileExportDocx,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        println!("Action: FileExportWord triggered!");

        // 1. Get the bytes from the preview panel
        let docx_bytes = self.preview_panel.read(cx).export_docx(); // <-- This line now calls your new method

        // 2. Determine default filename and directory
        let editor = self.editor_panel.read(cx);
        let (default_dir, default_name) = if let Some(path) = &editor.active_file_path {
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));

            let name = path
                .with_extension("docx")
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "document.docx".to_string());

            (dir, Some(name))
        } else {
            (
                std::path::PathBuf::from("."),
                Some("document.docx".to_string()),
            )
        };

        if let Some(bytes) = docx_bytes {
            cx.spawn(move |_, spawned_async_cx: &mut AsyncApp| {
                let cx_for_async_block = spawned_async_cx.clone();
                let bytes = bytes;
                let dir = default_dir;
                let name = default_name;

                async move {
                    let name_ref = name.as_deref();

                    let receiver = cx_for_async_block
                        .update(|app_cx| app_cx.prompt_for_new_path(&dir, name_ref));

                    match receiver.await {
                        Ok(Ok(Some(path))) => {
                            // Success: User picked a path
                            match std::fs::write(&path, &bytes) {
                                Ok(_) => println!("Successfully exported DOCX to {:?}", path),
                                Err(e) => eprintln!("Failed to save DOCX to {:?}: {}", path, e),
                            }
                        }
                        Ok(Ok(None)) => {
                            // Success: User cancelled the dialog
                            println!("Export cancelled by user.");
                        }
                        Ok(Err(e)) => {
                            // The prompt logic itself returned an error
                            eprintln!("Error during prompt interaction: {:?}", e);
                        }
                        Err(_) => {
                            // The oneshot channel was cancelled (e.g. sender dropped)
                            eprintln!("Prompt channel was closed unexpectedly.");
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
}
