//! Blueprints support

use blueprint::Orbit;

#[derive(Clone, Debug)]
pub struct Blueprints<const Layers: usize, O: Orbit> {
    layers: [O; Layers],
    app: O,
}

impl<const Layers: usize, O: Orbit> Blueprints<Layers, O> {
    /// Borrow the intermediate Layers as mutable
    pub fn layers_as_mut(&mut self) -> &mut [O; Layers] {
        &mut self.layers
    }
    /// Borrow the terminating App layer as mutable
    pub fn app_as_mut(&mut self) -> &mut O {
        &mut self.app
    }
    /// Count of all Layers +1 App
    pub fn count_all_layers(&self) -> usize {
        self.layers.len() + 1
    }
    /// Iterate mutable non-App layers
    pub fn layers_iter_mut<'a>(&'a mut self) -> impl Iterator + use<'a, Layers, O> {
        self.layers.iter_mut()
    }
}

/// Blueprints needs Layers constructor
pub struct BlueprintsLayers<const Layers: usize>;

impl<const Layers: usize> BlueprintsLayers<Layers> {
    /// Set the intermediate layers
    pub fn layers<O: Orbit>(orbits: [O; Layers]) -> BlueprintsNeedApp<Layers, O> {
        BlueprintsNeedApp { layers: orbits }
    }
    /// No intermediate layers involved
    pub fn no_layers<O: Orbit>() -> BlueprintsNeedApp<0, O> {
        BlueprintsNeedApp { layers: [] }
    }
}

/// Blueprints needs App constructor
pub struct BlueprintsNeedApp<const Layers: usize, O: Orbit> {
    layers: [O; Layers],
}

impl<const Layers: usize, O: Orbit> BlueprintsNeedApp<Layers, O> {
    /// Set the terminating App or final layer
    pub fn app(self, app: O) -> Blueprints<Layers, O> {
        Blueprints::<Layers, _> {
            layers: self.layers,
            app,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use blueprint::{Left, NoLeft, NoRight, Right};

    struct NoError;
    struct NoPosition;

    #[derive(Debug)]
    struct FirstLayer;
    #[derive(Debug)]
    struct SecondLayer;
    #[derive(Debug)]
    struct App;
    impl Orbit for FirstLayer {
        type Position = NoPosition;
        type Error = NoError;
        fn advance_with<B, L: Left, R: Right>(
            &mut self,
            _: &mut B,
            _: &mut L,
            _: &mut R,
        ) -> Result<Self::Position, Self::Error> {
            unreachable!()
        }
    }
    impl Orbit for SecondLayer {
        type Position = NoPosition;
        type Error = NoError;
        fn advance_with<B, L: Left, R: Right>(
            &mut self,
            _: &mut B,
            _: &mut L,
            _: &mut R,
        ) -> Result<Self::Position, Self::Error> {
            unreachable!()
        }
    }
    impl Orbit for App {
        type Position = NoPosition;
        type Error = NoError;
        fn advance_with<B, L: Left, R: Right>(
            &mut self,
            _: &mut B,
            _: &mut L,
            _: &mut R,
        ) -> Result<Self::Position, Self::Error> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    enum MyApp {
        First(FirstLayer),
        Second(SecondLayer),
        App(App),
    }

    impl Orbit for MyApp {
        type Position = NoPosition;
        type Error = NoError;
        fn advance_with<B, L: Left, R: Right>(
            &mut self,
            _: &mut B,
            _: &mut L,
            _: &mut R,
        ) -> Result<Self::Position, Self::Error> {
            unreachable!()
        }
    }

    #[test]
    fn construct_2_w_app() {
        let blueprints =
            BlueprintsLayers::<2>::layers([MyApp::First(FirstLayer), MyApp::Second(SecondLayer)])
                .app(MyApp::App(App));
    }
    #[test]
    fn construct_app_only() {
        let blueprints = BlueprintsLayers::<0>::no_layers().app(MyApp::App(App));
    }
}
