use std::cell::OnceCell;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{
    gdk, gio,
    glib::{self, clone},
    subclass::prelude::*,
};

use crate::{
    application::Application,
    config,
    models::{Account, OTPUri, ProvidersModel, SETTINGS, keyring},
    utils::spawn_tokio_blocking,
    widgets::{
        AccountAddDialog, ErrorRevealer,
        accounts::AccountDetailsPage,
        providers::{ProvidersList, ProvidersListView},
    },
};

pub enum View {
    Login,
    Accounts,
    Account(Account),
}

mod imp {
    use adw::subclass::prelude::*;
    use glib::subclass;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/com/belmoussaoui/Authenticator/window.ui")]
    #[properties(wrapper_type = super::Window)]
    pub struct Window {
        #[property(get, set, construct_only)]
        pub model: OnceCell<ProvidersModel>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub providers: TemplateChild<ProvidersList>,
        #[template_child]
        pub account_details: TemplateChild<AccountDetailsPage>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub navigation_view: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub error_revealer: TemplateChild<ErrorRevealer>,
        #[template_child]
        pub search_btn: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub password_entry: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub locked_status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub accounts_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub empty_status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub unlock_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "Window";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;
        type Interfaces = (gio::Initable,);

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();

            klass.install_action("win.search", None, |win, _, _| {
                let search_btn = &win.imp().search_btn;
                search_btn.set_active(!search_btn.is_active());
            });

            klass.install_action("win.add_account", None, |win, _, _| {
                win.open_add_account(None);
            });

            klass.install_action("win.back", None, |win, _, _| {
                // Always return back to accounts list
                win.set_view(View::Accounts);
            });

            klass.install_action("win.unlock", None, |win, _, _| {
                let imp = win.imp();
                let app = win.app();
                let password = imp.password_entry.text();
                let is_current_password = spawn_tokio_blocking(async move {
                    keyring::is_current_password(&password)
                        .await
                        .unwrap_or_else(|err| {
                            tracing::debug!("Could not verify password: {:?}", err);
                            false
                        })
                });
                if is_current_password {
                    imp.password_entry.set_text("");
                    app.set_is_locked(false);
                    app.restart_lock_timeout();
                    win.set_view(View::Accounts);
                    win.model().load();
                } else {
                    imp.error_revealer.popup(&gettext("Wrong Password"));
                }
            });
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let win = self.obj();
            self.providers.set_model(win.model());

            self.providers.model().connect_items_changed(clone!(
                #[weak]
                win,
                move |_, _, _, _| {
                    // We do a check on set_view to ensure we always use the right page
                    if !win.app().is_locked() {
                        win.set_view(View::Accounts);
                    }
                }
            ));

            win.set_icon_name(Some(config::APP_ID));
            self.empty_status_page.set_icon_name(Some(config::APP_ID));
            self.locked_status_page.set_icon_name(Some(config::APP_ID));

            // load latest window state
            let width = SETTINGS.window_width();
            let height = SETTINGS.window_height();

            if width > -1 && height > -1 {
                win.set_default_size(width, height);
            }

            let is_maximized = SETTINGS.is_maximized();
            if is_maximized {
                win.maximize();
            }
            self.account_details.set_providers_model(win.model());

            if config::PROFILE == "Devel" {
                win.add_css_class("devel");
            }
            win.set_view(View::Accounts);

            self.search_bar.connect_entry(&*self.search_entry);
        }
    }
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn enable_debugging(&self, toggle: bool) -> bool {
            if config::PROFILE != "Devel" {
                tracing::warn!("Inspector is disabled for non development builds");
                false
            } else {
                self.parent_enable_debugging(toggle)
            }
        }

        fn close_request(&self) -> glib::Propagation {
            if let Err(err) = self.obj().save_window_state() {
                tracing::warn!("Failed to save window state {:#?}", err);
            }
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
    impl InitableImpl for Window {
        // As the application property on gtk::ApplicationWindow is not marked
        // as CONSTRUCT, we need to implement Initable so we can run code once all the
        // object properties are set.
        fn init(&self, _cancellable: Option<&gio::Cancellable>) -> Result<(), glib::Error> {
            let win = self.obj();
            let app = win.app();
            win.action_set_enabled("win.add_account", !app.is_locked());
            app.connect_is_locked_notify(clone!(
                #[weak]
                win,
                move |app| {
                    let is_locked = app.is_locked();
                    win.action_set_enabled("win.add_account", !is_locked);
                    if is_locked {
                        win.set_view(View::Login);
                    } else {
                        win.set_view(View::Accounts);
                    };
                }
            ));
            if app.is_locked() {
                win.set_view(View::Login);
            }
            Ok(())
        }
    }
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::Initable, gio::ActionMap, gio::ActionGroup, gtk::Accessible,
                    gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager,
                    gtk::Native, gtk::Root;
}

#[gtk::template_callbacks]
impl Window {
    pub fn new(model: &ProvidersModel, app: &Application) -> Self {
        gio::Initable::builder()
            .property("application", app)
            .property("model", model)
            .build(gio::Cancellable::NONE)
            .unwrap()
    }

    pub fn set_view(&self, view: View) {
        let imp = self.imp();
        match view {
            View::Login => {
                self.set_default_widget(Some(&*imp.unlock_button));
                imp.main_stack.set_visible_child_name("login");
                imp.search_bar.set_key_capture_widget(gtk::Widget::NONE);
                imp.password_entry.grab_focus();
            }
            View::Accounts => {
                self.set_default_widget(gtk::Widget::NONE);
                imp.main_stack.set_visible_child_name("unlocked");
                imp.navigation_view.pop();
                if imp.providers.model().n_items() == 0 {
                    if self.model().has_providers() {
                        // We do have at least one provider
                        // the 0 items comes from the search filter, so let's show an empty search
                        // page instead
                        imp.providers.set_view(ProvidersListView::NoSearchResults);
                    } else {
                        imp.accounts_stack.set_visible_child_name("empty");
                        imp.search_bar.set_key_capture_widget(gtk::Widget::NONE);
                    }
                } else {
                    imp.providers.set_view(ProvidersListView::List);
                    imp.accounts_stack.set_visible_child_name("accounts");
                    imp.search_bar.set_key_capture_widget(Some(self));
                }
            }
            View::Account(account) => {
                self.set_default_widget(gtk::Widget::NONE);
                imp.search_bar.set_key_capture_widget(gtk::Widget::NONE);
                imp.main_stack.set_visible_child_name("unlocked");
                imp.navigation_view.push_by_tag("account");
                imp.account_details.set_account(&account);
            }
        }
    }

    pub fn add_toast(&self, toast: adw::Toast) {
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn open_add_account(&self, otp_uri: Option<&OTPUri>) {
        let model = self.model();
        let dialog = AccountAddDialog::new(&model);
        if let Some(uri) = otp_uri {
            dialog.set_from_otp_uri(uri);
        }

        dialog.connect_added(clone!(
            #[weak(rename_to = win)]
            self,
            move |_| {
                win.providers().refilter();
            }
        ));
        dialog.present(Some(self));
    }

    pub fn providers(&self) -> ProvidersList {
        self.imp().providers.clone()
    }

    fn should_handle_account_keys(&self) -> bool {
        let imp = self.imp();

        // The providers list is only mapped while the accounts list is visible.
        if !imp.providers.is_mapped() {
            return false;
        }

        let Some(focus) = gtk::prelude::RootExt::focus(self) else {
            return false;
        };

        let search_entry = imp.search_entry.clone().upcast::<gtk::Widget>();
        let providers = imp.providers.clone().upcast::<gtk::Widget>();

        // `GtkSearchEntry` focuses an internal text widget, so also check whether
        // the focused widget is a descendant of the search entry.
        focus == search_entry
            || focus.is_ancestor(&search_entry)
            || focus == providers
            || focus.is_ancestor(&providers)
    }

    fn app(&self) -> Application {
        self.application().and_downcast::<Application>().unwrap()
    }

    fn save_window_state(&self) -> anyhow::Result<()> {
        let size = self.default_size();
        SETTINGS.set_window_width(size.0)?;
        SETTINGS.set_window_height(size.1)?;
        SETTINGS.set_is_maximized(self.is_maximized())?;
        Ok(())
    }

    #[template_callback]
    fn on_password_entry_activate(&self) {
        WidgetExt::activate_action(self, "win.unlock", None).unwrap();
    }

    #[template_callback]
    fn on_account_removed(&self, account: Account) {
        let provider = account.provider();
        account.delete().unwrap();
        provider.remove_account(&account);
        self.providers().refilter();
        self.set_view(View::Accounts);
    }

    #[template_callback]
    fn on_provider_changed(&self) {
        self.providers().refilter();
    }

    #[template_callback]
    fn on_account_shared(&self, account: Account) {
        self.set_view(View::Account(account));
    }

    #[template_callback]
    fn on_gesture_click_pressed(&self) {
        self.app().restart_lock_timeout();
    }

    #[template_callback]
    fn on_key_pressed(
        &self,
        keyval: gdk::Key,
        _keycode: u32,
        state: gdk::ModifierType,
    ) -> glib::Propagation {
        self.app().restart_lock_timeout();

        if self.should_handle_account_keys() {
            let has_control = state.contains(gdk::ModifierType::CONTROL_MASK);
            let has_other_modifier = state.intersects(
                gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SUPER_MASK
                    | gdk::ModifierType::SHIFT_MASK,
            );

            if has_control && !has_other_modifier && keyval == gdk::Key::c {
                return if self.providers().copy_selected_otp() {
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                };
            }

            if !has_control && !has_other_modifier {
                match keyval {
                    gdk::Key::Down => {
                        return if self.providers().select_next() {
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        };
                    }
                    gdk::Key::Up => {
                        return if self.providers().select_previous() {
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        };
                    }
                    _ => (),
                }
            }
        }

        glib::Propagation::Proceed
    }

    #[template_callback]
    fn on_search_changed(&self, entry: &gtk::SearchEntry) {
        let text = entry.text().to_string();
        self.imp().providers.search(text);
    }

    #[template_callback]
    fn on_search_started(&self, _entry: &gtk::SearchEntry) {
        self.imp().search_btn.set_active(true);
    }

    #[template_callback]
    fn on_search_stopped(&self, _entry: &gtk::SearchEntry) {
        self.imp().search_btn.set_active(false);
    }
}
