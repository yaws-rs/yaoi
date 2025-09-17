//! Blueprints support

use blueprint::Orbit;

pub struct BlueprintsLayers<const Layers: usize>;

pub struct BlueprintsNeedApp<const Layers: usize, O: Orbit> {
    layers: [O; Layers],
}

#[derive(Debug)]
pub struct Blueprints<const Layers: usize, O: Orbit> {
    layers: [O; Layers],
    app: O,
}
impl<const Layers: usize> BlueprintsLayers<Layers> {
    pub fn layers<O: Orbit>(orbits: [O; Layers]) -> BlueprintsNeedApp<Layers, O> {
        BlueprintsNeedApp { layers: orbits }
    }
    pub fn no_layers<O: Orbit>() -> BlueprintsNeedApp<0, O> {
        BlueprintsNeedApp { layers: [] }
    }
}


impl<const Layers: usize, O: Orbit> BlueprintsNeedApp<Layers, O> {
    pub fn app(self, app: O) -> Blueprints<Layers, O> {
        Blueprints::<Layers, _>{ layers: self.layers, app }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use blueprint::{NoLeft, NoRight, Left, Right};

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
        fn advance_with<B, L: Left, R: Right>(&mut self,_: &mut B,_: &mut L,_: &mut R) -> Result<Self::Position, Self::Error> { unreachable!() }
    }
    impl Orbit for SecondLayer {
        type Position = NoPosition;
        type Error = NoError;
        fn advance_with<B, L: Left, R: Right>(&mut self,_: &mut B,_: &mut L,_: &mut R) -> Result<Self::Position, Self::Error> { unreachable!() }
    }
    impl Orbit for App {
        type Position = NoPosition;
        type Error = NoError;
        fn advance_with<B, L: Left, R: Right>(&mut self,_: &mut B,_: &mut L,_: &mut R) -> Result<Self::Position, Self::Error> { unreachable!() }
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
        fn advance_with<B, L: Left, R: Right>(&mut self,_: &mut B,_: &mut L,_: &mut R) -> Result<Self::Position, Self::Error> { unreachable!() }
    }
    
    #[test]
    fn construct_2_w_app() {
        let blueprints = BlueprintsLayers::<2>::layers([MyApp::First(FirstLayer), MyApp::Second(SecondLayer)]).app(MyApp::App(App));
    }
    #[test]
    fn construct_app_only() {
        let blueprints = BlueprintsLayers::<0>::no_layers().app(MyApp::App(App));
    }    
}
