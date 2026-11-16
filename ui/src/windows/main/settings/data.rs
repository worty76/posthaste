use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{CommandError, DataInfo};

use super::{Context, Row};
use crate::browser::bind_text;
use crate::components::{Button, Dialog, Variant};
use crate::ipc::api;
use crate::logic::attachments::format_bytes;
use crate::state::Message;
use crate::time;

/// The word the user types to confirm clearing everything. Not translated:
/// it is the same word the shell checks for.
const CONFIRM_WORD: &str = "DELETE";

/// A long-running thing this section does, to show which button is busy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Task {
    Export,
    Import,
    Backup,
    Clear,
}

/// Where the notes are kept, and getting them out, in, copied or erased.
#[component]
pub(super) fn DataSection(context: Context) -> impl IntoView {
    let i18n = context.i18n;
    let info = RwSignal::new(None::<DataInfo>);
    let busy = RwSignal::new(None::<Task>);
    let clear_open = RwSignal::new(false);
    let typed = RwSignal::new(String::new());

    let refresh = move || {
        spawn_local(async move { info.set(api::data_info().await.ok()) });
    };
    refresh();

    /// Runs a task, says what it did, and refreshes the numbers.
    async fn run(
        context: Context,
        busy: RwSignal<Option<Task>>,
        task: Task,
        work: impl std::future::Future<Output = Result<Option<Message>, CommandError>>,
    ) {
        busy.set(Some(task));
        match work.await {
            Ok(Some(done)) => context.toaster.info(done),
            // The user closed the file dialog.
            Ok(None) => {}
            Err(error) => context.toaster.fail(error),
        }
        busy.set(None);
    }
    let start = move |task: Task| {
        spawn_local(async move {
            match task {
                Task::Export => {
                    let work = async {
                        let written = api::data_export(time::utc_offset_minutes()).await?;
                        Ok(written
                            .map(|count| Message::count(key::SETTINGS_DATA_EXPORTED, count as u64)))
                    };
                    run(context, busy, task, work).await;
                }
                Task::Import => {
                    let work = async {
                        let Some(report) = api::data_import().await? else {
                            return Ok(None);
                        };
                        let imported = report.imported as u64;
                        let skipped = report.skipped_existing + report.skipped_invalid;
                        Ok(Some(if skipped == 0 {
                            Message::count(key::SETTINGS_DATA_IMPORTED, imported)
                        } else {
                            Message::Count(
                                key::SETTINGS_DATA_IMPORTED_SKIPPED,
                                imported,
                                vec![("skipped", skipped.to_string())],
                            )
                        }))
                    };
                    run(context, busy, task, work).await;
                }
                Task::Backup => {
                    let work = async {
                        let saved = api::data_backup().await?;
                        Ok(saved.map(|_| Message::Plain(key::SETTINGS_DATA_BACKED_UP)))
                    };
                    run(context, busy, task, work).await;
                }
                Task::Clear => {
                    let work = async {
                        // Clearing resets the settings too.
                        context.settings.accept(api::data_clear().await?);
                        clear_open.set(false);
                        typed.set(String::new());
                        Ok(Some(Message::Plain(key::SETTINGS_DATA_CLEARED)))
                    };
                    run(context, busy, task, work).await;
                }
            }
            refresh();
        });
    };
    let reveal = move |()| {
        spawn_local(async move {
            if let Err(error) = api::data_reveal().await {
                context.toaster.fail(error);
            }
        });
    };
    let close_clear = move |()| {
        clear_open.set(false);
        typed.set(String::new());
    };
    let is_busy = move |task: Task| Signal::derive(move || busy.get() == Some(task));
    let confirm_label = move || {
        i18n.translator()
            .format(key::SETTINGS_DATA_CLEAR_CONFIRM, &[("word", CONFIRM_WORD)])
    };

    view! {
        <section class="settings__section" aria-labelledby="s-data">
            <h2 class="settings__heading" id="s-data">{i18n.text(key::SETTINGS_DATA_HEADING)}</h2>
            <div class="row row--stack">
                <div class="row__text">
                    <div class="row__label">{i18n.text(key::SETTINGS_DATA_DATABASE)}</div>
                    <div class="row__help">
                        {i18n.text(key::SETTINGS_DATA_DATABASE_HELP)}
                        {move || info.with(|info| info.as_ref().map(|info| {
                            let size = format_bytes(info.size_bytes);
                            let count = u64::try_from(info.note_count).unwrap_or(0);
                            let text = i18n.translator().plural(
                                key::SETTINGS_DATA_DATABASE_SIZE,
                                count,
                                &[("size", &size)],
                            );
                            format!(" {text}")
                        }))}
                    </div>
                </div>
                <div class="path selectable">
                    {move || info.with(|info| match info {
                        Some(info) => info.path.clone(),
                        None => i18n.translator().text(key::SETTINGS_DATA_UNAVAILABLE).to_owned(),
                    })}
                </div>
                <div class="row__control">
                    <Button on_click=reveal>{i18n.text(key::SETTINGS_DATA_REVEAL)}</Button>
                </div>
            </div>
            <Row i18n=i18n label=key::SETTINGS_DATA_EXPORT help=key::SETTINGS_DATA_EXPORT_HELP>
                <Button busy=is_busy(Task::Export) on_click=move |()| start(Task::Export)>
                    {i18n.text(key::SETTINGS_DATA_EXPORT_ACTION)}
                </Button>
            </Row>
            <Row i18n=i18n label=key::SETTINGS_DATA_IMPORT help=key::SETTINGS_DATA_IMPORT_HELP>
                <Button busy=is_busy(Task::Import) on_click=move |()| start(Task::Import)>
                    {i18n.text(key::SETTINGS_DATA_IMPORT_ACTION)}
                </Button>
            </Row>
            <Row i18n=i18n label=key::SETTINGS_DATA_BACKUP help=key::SETTINGS_DATA_BACKUP_HELP>
                <Button busy=is_busy(Task::Backup) on_click=move |()| start(Task::Backup)>
                    {i18n.text(key::SETTINGS_DATA_BACKUP_ACTION)}
                </Button>
            </Row>
            <Row i18n=i18n label=key::SETTINGS_DATA_CLEAR help=key::SETTINGS_DATA_CLEAR_HELP>
                <Button variant=Variant::Danger on_click=move |()| clear_open.set(true)>
                    {i18n.text(key::SETTINGS_DATA_CLEAR_ACTION)}
                </Button>
            </Row>

            <Dialog
                open=clear_open
                title=i18n.label(key::SETTINGS_DATA_CLEAR_TITLE)
                on_close=close_clear
                actions=Box::new(move || view! {
                    <Button on_click=close_clear>{i18n.text(key::COMMON_CANCEL)}</Button>
                    <Button
                        variant=Variant::Danger
                        busy=is_busy(Task::Clear)
                        disabled=Signal::derive(move || typed.with(|typed| typed != CONFIRM_WORD))
                        on_click=move |()| start(Task::Clear)
                    >
                        {i18n.text(key::SETTINGS_DATA_CLEAR_EVERYTHING)}
                    </Button>
                }.into_any())
            >
                <p class="dialog__text">{i18n.text(key::SETTINGS_DATA_CLEAR_TEXT)}</p>
                <input
                    class="field"
                    aria-label=confirm_label
                    placeholder=confirm_label
                    autocomplete="off"
                    spellcheck="false"
                    use:bind_text=Signal::derive(move || typed.get())
                    on:input=move |event| typed.set(event_target_value(&event))
                />
            </Dialog>
        </section>
    }
}
