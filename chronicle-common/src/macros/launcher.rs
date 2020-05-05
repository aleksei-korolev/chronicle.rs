#[macro_export]
macro_rules! launcher {
    (
        apps_builder: $name:ident {$($app:ident : $t:ty),+},
        apps: $apps:ident {$($field:ident : $type:ty),*}
    ) => {

        enum Event {
            StartApp(String),
            ShutdownApp(String),
            AknShutdown(String),
            RegisterApp(String, Box<dyn ShutdownTx>),
            RegisterDashboard(String, Box<dyn DashboardTx>),
            AppsStatus(String),
            Break,
        }

        launcher!(
            apps_builder: $name {$($app : $t),+},
            apps: $apps {$($field : $type),*},
            event: Event
        );

        impl LauncherEvent for Event {
            fn start_app(app_name: String) -> Event {
                Event::StartApp(app_name)
            }
            fn apps_status(dashboard_name: String) -> Event {
                Event::AppsStatus(dashboard_name)
            }
            fn shutdown_app(app_name: String) -> Event {
                Event::ShutdownApp(app_name)
            }
            fn aknowledge_shutdown(app_name: String) -> Event {
                Event::AknShutdown(app_name)
            }
            fn register_dashboard(dashboard_name: String, dashboard_tx: Box<dyn DashboardTx>) -> Event {
                Event::RegisterDashboard(dashboard_name, dashboard_tx)
            }
            fn register_app(app_name: String, shutdown_tx: Box<dyn ShutdownTx>) -> Event {
                Event::RegisterApp(app_name, shutdown_tx)
            }
            fn break_launcher() -> Event {
                Event::Break
            }
        }
        // TODO implement basic strategies for Apps {}
        impl Apps {
            async fn one_for_one(mut self) {
                while let Some(event) = self.rx.0.recv().await {
                    match event {
                        Event::RegisterDashboard(dashboard_name, dashboard_tx) => {
                            // register dashboard by adding it to map
                            self.dashboards.insert(dashboard_name, dashboard_tx);
                        }
                        Event::RegisterApp(app_name, shutdown_tx) => {
                            // register app in map
                            self.apps.insert(app_name.clone(), shutdown_tx);
                            // tell dashboard(s) that we startedapp
                            for (_dashboard_name, dashboard_tx) in self.dashboards.iter_mut() {
                                // aknowledge startedapp
                                dashboard_tx.started_app(app_name.clone());
                            }
                        }
                        Event::StartApp(app_name) => {
                            self.start_app(app_name.clone()).await;
                            // tell dashboards that we started app
                            for (_, mut dashboard_tx) in &mut self.dashboards {
                                dashboard_tx.started_app(app_name.clone());
                            }
                        }
                        Event::ShutdownApp(app_name) => {
                            if let Some(shutdown_tx) = self.apps.remove(&app_name) {
                                shutdown_tx.shutdown();
                            };
                        }
                        Event::AknShutdown(app_name) => {
                            // aknowledging shutdown for an app under one_for_one policy require us to restart the app
                            // check if the the shutdown was requested
                            if let Some(_shutdown_tx) = self.apps.remove(&app_name) {
                                // mean the shutdown it's not requested so we restart
                                self.start_app(app_name.clone()).await;
                                // tell dashboards that we started app
                                for (_, dashboard_tx) in &mut self.dashboards {
                                    dashboard_tx.started_app(app_name.clone());
                                }
                            } else {
                                // tell dashboards that we shutdown an app
                                for (_, dashboard_tx) in &mut self.dashboards {
                                    dashboard_tx.shutdown_app(app_name.clone());
                                }
                            }
                        }
                        Event::Break => {
                            // break launcher
                            break
                        }
                        _ => {
                            // todo app_status
                            todo!()
                        }
                    }
                }
            }
        }
    };
    (
        apps_builder: $name:ident {$($app:ident : $t:ty),+},
        apps: $apps:ident {$($field:ident : $type:ty),*},
        event: $event:ty
    ) => {
        use tokio::sync::mpsc;
        use std::collections::HashMap;
        use chronicle_common::traits::{
            launcher::{
                LauncherTx,
            },
            shutdown::ShutdownTx,
            dashboard::DashboardTx,
        };
        pub trait LauncherEvent: Send {
            fn start_app(app_name: String) -> Self;
            fn shutdown_app(app_name: String) -> Self;
            fn aknowledge_shutdown(app_name: String) -> Self;
            fn register_app(app_name: String, shutdown_tx: Box<dyn ShutdownTx>) -> Self;
            fn register_dashboard(dashboard_name: String, dashboard_tx: Box<dyn DashboardTx>) -> Self;
            fn apps_status(dashboard_name: String) -> Self;
            fn break_launcher() -> Self;
        }
        #[derive(Clone)]
        pub struct Sender(mpsc::UnboundedSender<$event>);
        pub struct Receiver(mpsc::UnboundedReceiver<$event>);
        impl LauncherTx for Sender {
            fn start_app(&mut self, app_name: String) {
                let _ = self.0.send(LauncherEvent::start_app(app_name));
            }
            fn shutdown_app(&mut self, app_name: String) {
                let _ = self.0.send(LauncherEvent::shutdown_app(app_name));
            }
            fn aknowledge_shutdown(&mut self, app_name: String) {
                let _ = self.0.send(LauncherEvent::aknowledge_shutdown(app_name));
            }
            fn register_dashboard(&mut self, dashboard_name: String, dashboard_tx: Box<dyn DashboardTx>) {
                let _ = self.0.send(LauncherEvent::register_dashboard(dashboard_name, dashboard_tx));
            }
            fn register_app(&mut self, app_name: String, shutdown_tx: Box<dyn ShutdownTx>) {
                let _ = self.0.send(LauncherEvent::register_app(app_name, shutdown_tx));
            }
            fn apps_status(&mut self, dashboard_name: String) {
                let _ = self.0.send(LauncherEvent::apps_status(dashboard_name));
            }
            fn break_launcher(&mut self) {
                let _ = self.0.send(LauncherEvent::break_launcher());
            }
        }
        #[derive(Default)]
        pub struct $name {
            tx: Option<Sender>,
            rx: Option<Receiver>,
            $(
                $app: Option<$t>,
            )*
        }
        pub struct $apps {
            dashboards: HashMap<String, Box<dyn DashboardTx>>,
            apps: HashMap<String, Box<dyn ShutdownTx>>,
            app_count: usize,
            tx: Sender,
            rx: Receiver,
            $(
                $app: Option<$t>,
            )*
            $(
                $field: Option<$type>,
            )*
        }
        impl $apps {
            $(
                async fn $app(mut self) -> Self {
                    self.$app.clone().take().unwrap().build().run().await;
                    self
                }
            )*
            $(
                pub fn $field(mut self, $field: $type) -> Self {
                    self.$field.replace($field);
                    self
                }
            )*
            async fn start_app(&mut self, app_name: String) {
                match &app_name[..] {
                    $(
                        stringify!($app) => {
                            self.$app.clone().take().unwrap().build().run().await;
                        }
                    )*
                    _ => {unreachable!()}
                }
            }
        }
        impl $name {
            pub fn new() -> Self {
                let (tx, rx) = mpsc::unbounded_channel::<$event>();
                let mut launcher = Self::default();
                launcher.tx.replace(Sender(tx));
                launcher.rx.replace(Receiver(rx));
                launcher
            }

            pub fn clone_tx(&self) -> Sender {
                self.tx.as_ref().unwrap().clone()
            }

            pub fn to_apps(mut self) -> $apps {
                let tx = self.tx.take().unwrap();
                let rx = self.rx.take().unwrap();
                $apps {
                    dashboards: HashMap::new(),
                    apps: HashMap::new(),
                    app_count: self.app_count(),
                    tx,
                    rx,
                    $(
                        $app: self.$app,
                    )*
                    $(
                        $field: None,
                    )*
                }
            }
            $(
                pub fn $app(mut self, $app: $t) -> Self {
                    self.$app.replace($app.launcher_tx(Box::new(self.clone_tx())));
                    self
                }
            )*

            fn app_count(&self) -> usize {
                launcher!(@count $($app),+)
            }
        }

    };
    (@count $t1:tt, $($t:tt),+) => { 1 + launcher!(@count $($t),+) };
    (@count $t:tt) => { 1 };
}
