use gettextrs::gettext;
use gtk::{gdk, glib, prelude::*};

use crate::{models::Account, widgets::Window};

mod imp {
    use std::cell::OnceCell;

    use adw::subclass::prelude::*;
    use glib::subclass;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::AccountRow)]
    #[template(resource = "/com/belmoussaoui/Authenticator/account_row.ui")]
    pub struct AccountRow {
        #[property(get, set, construct_only)]
        pub account: OnceCell<Account>,
        #[template_child]
        pub increment_btn: TemplateChild<gtk::Button>,
        #[template_child]
        pub otp_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AccountRow {
        const NAME: &'static str = "AccountRow";
        type Type = super::AccountRow;
        type ParentType = adw::ActionRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.add_binding_action(
                gdk::Key::c,
                gdk::ModifierType::CONTROL_MASK,
                "account.copy-otp",
            );

            klass.install_action("account.copy-otp", None, |row, _, _| {
                row.copy_otp();
            });
            klass.install_action("account.increment-counter", None, |row, _, _| {
                match row.account().increment_counter() {
                    Ok(_) => row.account().generate_otp(),
                    Err(err) => tracing::error!("Failed to increment the counter {err}"),
                };
            });
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AccountRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let account = obj.account();
            account
                .bind_property("name", &*obj, "title")
                .sync_create()
                .build();

            account
                .bind_property("name", &*obj, "tooltip-text")
                .sync_create()
                .build();

            account
                .bind_property("code", &*self.otp_label, "label")
                .sync_create()
                .build();
            self.otp_label.set_direction(gtk::TextDirection::Ltr);

            // Only display the increment button if it is a HOTP account
            self.increment_btn
                .set_visible(account.provider().method().is_event_based());
        }
    }
    impl WidgetImpl for AccountRow {}
    impl ListBoxRowImpl for AccountRow {}
    impl PreferencesRowImpl for AccountRow {}
    impl ActionRowImpl for AccountRow {}
}

glib::wrapper! {
    pub struct AccountRow(ObjectSubclass<imp::AccountRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl AccountRow {
    pub fn new(account: &Account) -> Self {
        glib::Object::builder().property("account", account).build()
    }

    /// Copy the one-time password to the clipboard and show a confirmation toast.
    pub fn copy_otp(&self) {
        self.account().copy_otp();

        if let Some(window) = self.root().and_downcast::<Window>() {
            let toast = adw::Toast::new(&gettext("One-Time password copied"));
            toast.set_timeout(3);
            window.add_toast(toast);
        }
    }

    /// Mark this row as the currently selected account in the list.
    pub fn set_selected(&self, selected: bool) {
        if selected {
            self.add_css_class("account-selected");
        } else {
            self.remove_css_class("account-selected");
        }
    }
}
