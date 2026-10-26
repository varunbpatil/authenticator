use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use crate::{
    models::{Account, Provider, ProvidersModel},
    widgets::{accounts::AccountRow, providers::ProviderRow},
};

pub enum ProvidersListView {
    NoSearchResults,
    List,
}

mod imp {
    use std::{cell::RefCell, sync::LazyLock};

    use glib::subclass::Signal;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/belmoussaoui/Authenticator/providers_list.ui")]
    pub struct ProvidersList {
        pub filter_model: gtk::FilterListModel,
        pub sorter: gtk::StringSorter,
        pub selected_row: RefCell<Option<AccountRow>>,
        #[template_child]
        pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub providers_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProvidersList {
        const NAME: &'static str = "ProvidersList";
        type Type = super::ProvidersList;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ProvidersList {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widget();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: LazyLock<Vec<Signal>> = LazyLock::new(|| {
                vec![
                    Signal::builder("shared")
                        .param_types([Account::static_type()])
                        .action()
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for ProvidersList {}
    impl BoxImpl for ProvidersList {}
}

glib::wrapper! {
    pub struct ProvidersList(ObjectSubclass<imp::ProvidersList>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ProvidersList {
    pub fn set_view(&self, view: ProvidersListView) {
        let imp = self.imp();
        match view {
            ProvidersListView::NoSearchResults => {
                self.clear_selection();
                imp.stack.set_visible_child_name("no-results");
            }
            ProvidersListView::List => {
                imp.stack.set_visible_child_name("results");
                self.ensure_selection();
            }
        }
    }

    /// Initialize the ProvidersList by setting the model to use.
    ///
    /// The model contains initially all the providers and are filtered
    /// to keep only the ones that have at least an account.
    pub fn set_model(&self, model: ProvidersModel) {
        let imp = self.imp();
        let accounts_filter = gtk::CustomFilter::new(move |object| {
            let provider = object.downcast_ref::<Provider>().unwrap();
            provider.has_accounts()
        });
        imp.filter_model.set_filter(Some(&accounts_filter));
        imp.filter_model.set_model(Some(&model));
    }

    pub fn refilter(&self) {
        let imp = self.imp();

        if let Some(filter) = imp.filter_model.filter() {
            filter.changed(gtk::FilterChange::Different);
        }
        imp.sorter.changed(gtk::SorterChange::Different);
        self.ensure_selection();
    }

    /// Returns an instance of the filtered initial model
    pub fn model(&self) -> gtk::FilterListModel {
        self.imp().filter_model.clone()
    }

    pub fn search(&self, text: String) {
        let accounts_filter = gtk::CustomFilter::new(move |object| {
            let provider = object.downcast_ref::<Provider>().unwrap();
            provider.filter(text.clone());
            provider.accounts().n_items() != 0
        });
        self.imp().filter_model.set_filter(Some(&accounts_filter));
        self.select_first();

        // The rows are re-created when the filter changes. Make sure one of the
        // new rows is selected once the model has been updated.
        glib::idle_add_local_once(clone!(
            #[weak(rename_to = list)]
            self,
            move || list.ensure_selection()
        ));
    }

    /// Select the first account in the list.
    pub fn select_first(&self) {
        if let Some(row) = self.account_rows().first() {
            self.select_row(row);
        } else {
            self.clear_selection();
        }
    }

    /// Select the account after the currently selected one.
    pub fn select_next(&self) -> bool {
        self.select_relative(1)
    }

    /// Select the account before the currently selected one.
    pub fn select_previous(&self) -> bool {
        self.select_relative(-1)
    }

    /// Copy the one-time password of the selected account.
    ///
    /// Returns whether an account was available to copy from.
    pub fn copy_selected_otp(&self) -> bool {
        let rows = self.account_rows();

        if rows.is_empty() {
            self.clear_selection();
            return false;
        }

        let row = self
            .selected_row(&rows)
            .unwrap_or_else(|| rows.first().unwrap().clone());

        self.select_row(&row);
        row.copy_otp();

        true
    }

    fn select_relative(&self, delta: isize) -> bool {
        let rows = self.account_rows();

        if rows.is_empty() {
            self.clear_selection();
            return false;
        }

        let position = match self.selected_row(&rows) {
            Some(row) => rows
                .iter()
                .position(|candidate| candidate.account().id() == row.account().id())
                .unwrap_or(0),
            None if delta < 0 => rows.len() - 1,
            None => 0,
        };

        let new_position = if delta < 0 {
            position.saturating_sub(delta.unsigned_abs())
        } else {
            (position + delta as usize).min(rows.len() - 1)
        };

        let row = &rows[new_position];
        self.select_row(row);
        // Move the focus with the selection. Otherwise, returning to the window
        // can restore focus to an old row and select it again.
        row.grab_focus();

        true
    }

    fn selected_row(&self, rows: &[AccountRow]) -> Option<AccountRow> {
        let selected = self.imp().selected_row.borrow();
        let selected = selected.as_ref()?;
        let id = selected.account().id();

        rows.iter().find(|row| row.account().id() == id).cloned()
    }

    fn select_row(&self, row: &AccountRow) {
        if let Some(selected) = self.imp().selected_row.take() {
            selected.set_selected(false);
        }

        row.set_selected(true);
        self.imp().selected_row.replace(Some(row.clone()));
        self.scroll_to_row(row);
    }

    fn scroll_to_row(&self, row: &AccountRow) {
        // Wait until the list has been laid out before computing the row position.
        glib::idle_add_local_once(clone!(
            #[weak(rename_to = list)]
            self,
            #[weak]
            row,
            move || list.scroll_row_into_view(&row)
        ));
    }

    fn scroll_row_into_view(&self, row: &AccountRow) {
        let imp = self.imp();
        let Some(bounds) = row.compute_bounds(&*imp.stack) else {
            return;
        };

        let adjustment = imp.scrolled_window.vadjustment();
        let page_size = adjustment.page_size();

        if page_size <= 0.0 {
            return;
        }

        let account_top = bounds.y() as f64;
        let account_bottom = account_top + bounds.height() as f64;
        let visible_top = adjustment.value();
        let visible_bottom = visible_top + page_size;

        if account_top >= visible_top && account_bottom <= visible_bottom {
            return;
        }

        let margin = 6.0;
        let heading_top = Self::provider_row_for(row)
            .and_then(|provider_row| provider_row.compute_bounds(&*imp.stack))
            .map(|bounds| bounds.y() as f64)
            .unwrap_or(account_top);

        if account_top < visible_top {
            // Scrolling up: include the provider heading if it can be shown
            // together with the selected account.
            let target = heading_top - margin;
            let account_target = account_top - margin;

            if account_bottom <= target + page_size {
                adjustment.set_value(target);
            } else {
                adjustment.set_value(account_target);
            }
        } else {
            // Scrolling down: show as much of the provider heading as possible
            // while keeping the selected account visible.
            adjustment.set_value(account_bottom + margin - page_size);
        }
    }

    fn provider_row_for(row: &AccountRow) -> Option<ProviderRow> {
        let mut parent = row.parent();

        while let Some(widget) = parent {
            if let Some(provider_row) = widget.downcast_ref::<ProviderRow>() {
                return Some(provider_row.clone());
            }
            parent = widget.parent();
        }

        None
    }

    fn clear_selection(&self) {
        if let Some(selected) = self.imp().selected_row.take() {
            selected.set_selected(false);
        }
    }

    fn ensure_selection(&self) {
        let rows = self.account_rows();

        if rows.is_empty() {
            self.clear_selection();
            return;
        }

        if let Some(row) = self.selected_row(&rows) {
            self.select_row(&row);
        } else {
            self.select_row(&rows[0]);
        }
    }

    fn account_rows(&self) -> Vec<AccountRow> {
        let mut rows = Vec::new();
        let mut child = self.imp().providers_list.first_child();

        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(provider_row) = widget.downcast_ref::<ProviderRow>() {
                rows.extend(provider_row.account_rows());
            }
        }

        rows
    }

    fn setup_widget(&self) {
        let imp = self.imp();

        imp.sorter.set_ignore_case(true);
        imp.sorter
            .set_expression(Some(Provider::this_expression("name")));

        let sort_model =
            gtk::SortListModel::new(Some(imp.filter_model.clone()), Some(imp.sorter.clone()));

        imp.providers_list.bind_model(
            Some(&sort_model),
            clone!(
                #[strong(rename_to = list)]
                self,
                move |obj| {
                    let provider = obj.downcast_ref::<Provider>().unwrap();
                    let row = ProviderRow::new(provider);
                    row.connect_changed(clone!(
                        #[weak]
                        list,
                        move |_| {
                            list.refilter();
                        }
                    ));
                    row.connect_shared(clone!(
                        #[weak]
                        list,
                        move |_, account| {
                            list.emit_by_name::<()>("shared", &[&account]);
                        }
                    ));
                    row.connect_account_focused(clone!(
                        #[weak]
                        list,
                        move |_, account_row| {
                            list.select_row(&account_row);
                        }
                    ));

                    row.upcast::<gtk::Widget>()
                }
            ),
        );
    }
}
