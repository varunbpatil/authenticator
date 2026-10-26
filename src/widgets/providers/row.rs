use std::time::{SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use gtk::{
    glib::{self, clone},
    subclass::prelude::*,
};

use crate::{
    models::{Account, Provider},
    widgets::{ProgressIcon, ProviderImage, accounts::AccountRow},
};

mod imp {
    use std::{cell::OnceCell, sync::LazyLock};

    use glib::subclass::Signal;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/belmoussaoui/Authenticator/provider_row.ui")]
    #[properties(wrapper_type = super::ProviderRow)]
    pub struct ProviderRow {
        #[property(get, set, construct_only)]
        pub provider: OnceCell<Provider>,
        #[template_child]
        pub image: TemplateChild<ProviderImage>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub accounts_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub progress_icon: TemplateChild<ProgressIcon>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProviderRow {
        const NAME: &'static str = "ProviderRow";
        type Type = super::ProviderRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ProviderRow {
        fn signals() -> &'static [Signal] {
            static SIGNALS: LazyLock<Vec<Signal>> = LazyLock::new(|| {
                vec![
                    Signal::builder("changed").action().build(),
                    Signal::builder("account-focused")
                        .param_types([AccountRow::static_type()])
                        .action()
                        .build(),
                    Signal::builder("shared")
                        .param_types([Account::static_type()])
                        .action()
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widget();
        }
    }
    impl WidgetImpl for ProviderRow {}
    impl ListBoxRowImpl for ProviderRow {}
}

glib::wrapper! {
    pub struct ProviderRow(ObjectSubclass<imp::ProviderRow>)
        @extends gtk::Widget, gtk::ListBoxRow,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl ProviderRow {
    pub fn new(provider: &Provider) -> Self {
        glib::Object::builder()
            .property("provider", provider)
            .build()
    }

    pub fn connect_changed<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self) + 'static,
    {
        self.connect_local(
            "changed",
            false,
            clone!(
                #[weak(rename_to = row)]
                self,
                #[upgrade_or]
                None,
                move |_| {
                    callback(&row);
                    None
                }
            ),
        )
    }

    pub fn connect_shared<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, Account) + 'static,
    {
        self.connect_local(
            "shared",
            false,
            clone!(
                #[weak(rename_to = row)]
                self,
                #[upgrade_or]
                None,
                move |args| {
                    let account = args[1].get::<Account>().unwrap();
                    callback(&row, account);
                    None
                }
            ),
        )
    }

    pub fn connect_account_focused<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, AccountRow) + 'static,
    {
        self.connect_local(
            "account-focused",
            false,
            clone!(
                #[weak(rename_to = row)]
                self,
                #[upgrade_or]
                None,
                move |args| {
                    let account_row = args[1].get::<AccountRow>().unwrap();
                    callback(&row, account_row);
                    None
                }
            ),
        )
    }

    /// Returns the account rows in the order they are displayed.
    pub fn account_rows(&self) -> Vec<AccountRow> {
        let mut rows = Vec::new();
        let mut child = self.imp().accounts_list.first_child();

        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(row) = widget.downcast_ref::<AccountRow>() {
                rows.push(row.clone());
            }
        }

        rows
    }

    fn tick_progressbar(&self) {
        let imp = self.imp();
        let period_millis = self.provider().period() as u128 * 1000;
        let now: u128 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let remaining_time: u128 = period_millis - now % period_millis;

        let progress_fraction: f64 = (remaining_time as f64) / (period_millis as f64);

        imp.progress_icon.set_progress(progress_fraction as f32);
    }

    fn setup_widget(&self) {
        let imp = self.imp();
        let provider = self.provider();

        self.add_css_class(&provider.method().to_string());

        imp.image.set_provider(Some(&provider));
        if provider.method().is_event_based() {
            imp.progress_icon.set_visible(false);
        } else {
            // Update the progress bar whnever the remaining-time is updated
            self.tick_progressbar();
            provider.connect_remaining_time_notify(clone!(
                #[weak(rename_to = row)]
                self,
                move |_| {
                    row.tick_progressbar();
                }
            ));
        }

        provider
            .bind_property("name", &*imp.name_label, "label")
            .sync_create()
            .build();

        let sorter = gtk::StringSorter::builder()
            .ignore_case(true)
            .expression(Account::this_expression("name"))
            .build();
        let sort_model =
            gtk::SortListModel::new(Some(provider.accounts().clone()), Some(sorter.clone()));

        let create_callback = clone!(
            #[strong(rename_to = provider_row)]
            self,
            #[strong]
            sorter,
            move |account: &glib::Object| {
                let account = account.downcast_ref::<Account>().unwrap();
                let row = AccountRow::new(account);

                let focus_controller = gtk::EventControllerFocus::new();
                focus_controller.connect_enter(clone!(
                    #[weak]
                    provider_row,
                    #[weak]
                    row,
                    move |_| {
                        provider_row.emit_by_name::<()>("account-focused", &[&row]);
                    }
                ));
                row.add_controller(focus_controller);

                row.connect_activated(clone!(
                    #[weak]
                    account,
                    #[weak]
                    provider_row,
                    move |_| {
                        provider_row.emit_by_name::<()>("shared", &[&account]);
                    }
                ));

                account.connect_name_notify(clone!(
                    #[weak]
                    provider_row,
                    #[weak]
                    sorter,
                    move |_| {
                        // Re-sort in case the name was updated
                        sorter.changed(gtk::SorterChange::Different);
                        provider_row.emit_by_name::<()>("changed", &[]);
                    }
                ));
                row.upcast::<gtk::Widget>()
            }
        );

        imp.accounts_list
            .bind_model(Some(&sort_model), create_callback);
    }
}
