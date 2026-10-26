//! A level flight: speed, altitude and mass, and the conditions the wing
//! and its airfoil meet there.

use serde::{Deserialize, Serialize};

use crate::atmosphere::{Atmosphere, G0, isa};
use crate::error::{Result, positive};
use crate::wing::{Wing, WingConstraint};

/// A steady level flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    /// Airspeed, m/s.
    pub speed: f64,
    /// Geometric altitude, m.
    pub altitude: f64,
    /// Deviation from the standard temperature, K (0 for the standard day).
    #[serde(default)]
    pub temperature_offset: f64,
    /// Mass of the aircraft, kg.
    pub mass: f64,
}

/// What the wing and its airfoil meet in a level flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Conditions {
    /// The air at the flight's altitude.
    pub atmosphere: Atmosphere,
    /// Airspeed, m/s.
    pub speed: f64,
    /// Dynamic pressure ½ρV², Pa.
    pub dynamic_pressure: f64,
    /// Reynolds number on the mean chord, V c / ν.
    pub reynolds: f64,
    /// Mach number, V / a.
    pub mach: f64,
    /// Weight, N; in level flight the wing lifts exactly this much.
    pub weight: f64,
    /// Wing area, m².
    pub wing_area: f64,
    /// Aspect ratio of the wing.
    pub aspect_ratio: f64,
    /// Wing loading, weight / area, N/m².
    pub wing_loading: f64,
    /// Lift coefficient the wing must fly at, weight / (q S).
    pub required_cl: f64,
}

impl Flight {
    /// Checks the inputs and returns the air at the flight's altitude.
    pub fn atmosphere(&self) -> Result<Atmosphere> {
        positive("speed", self.speed)?;
        positive("mass", self.mass)?;
        isa(self.altitude, self.temperature_offset)
    }

    /// Weight, N.
    pub fn weight(&self) -> f64 {
        self.mass * G0
    }

    /// The conditions met by `wing` in this flight.
    pub fn conditions(&self, wing: &Wing) -> Result<Conditions> {
        wing.validate()?;
        let atmosphere = self.atmosphere()?;
        let dynamic_pressure = 0.5 * atmosphere.density * self.speed * self.speed;
        let weight = self.weight();
        let wing_area = wing.area();
        Ok(Conditions {
            atmosphere,
            speed: self.speed,
            dynamic_pressure,
            reynolds: self.speed * wing.chord / atmosphere.kinematic_viscosity,
            mach: self.speed / atmosphere.speed_of_sound,
            weight,
            wing_area,
            aspect_ratio: wing.aspect_ratio(),
            wing_loading: weight / wing_area,
            required_cl: weight / (dynamic_pressure * wing_area),
        })
    }

    /// The wing that flies this flight at the lift coefficient `cl`, with
    /// the span or the aspect ratio given by `constraint`.
    pub fn size_wing(&self, cl: f64, constraint: &WingConstraint) -> Result<Wing> {
        positive("lift coefficient", cl)?;
        let atmosphere = self.atmosphere()?;
        let dynamic_pressure = 0.5 * atmosphere.density * self.speed * self.speed;
        constraint.wing(self.weight() / (dynamic_pressure * cl))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flight of the request that started the mission feature:
    /// 20 km/h, 100 m high.
    fn slow_flyer(mass: f64) -> Flight {
        Flight {
            speed: 20.0 / 3.6,
            altitude: 100.0,
            temperature_offset: 0.0,
            mass,
        }
    }

    #[test]
    fn slow_flyer_conditions() {
        let c = slow_flyer(0.3)
            .conditions(&Wing {
                span: 1.0,
                chord: 0.15,
            })
            .unwrap();
        // Hand values (ISA at 100 m: ρ = 1.2133 kg/m³, ν = 1.472e-5 m²/s).
        assert!((c.atmosphere.density - 1.2133).abs() < 1e-4);
        assert!((c.dynamic_pressure - 18.72).abs() < 0.01);
        assert!((c.reynolds - 56_600.0).abs() < 100.0);
        assert!((c.mach - 0.0163).abs() < 1e-4);
        assert!((c.required_cl - 1.05).abs() < 0.005);
        assert!((c.wing_loading - 0.3 * G0 / 0.15).abs() < 1e-12);
        assert!((c.aspect_ratio - 1.0 / 0.15).abs() < 1e-12);
        // Lift equals weight.
        assert!((c.required_cl * c.dynamic_pressure * c.wing_area - c.weight).abs() < 1e-12);
    }

    #[test]
    fn a_larger_wing_flies_at_a_lower_cl_and_a_higher_reynolds() {
        let small = slow_flyer(0.3)
            .conditions(&Wing {
                span: 1.0,
                chord: 0.15,
            })
            .unwrap();
        let large = slow_flyer(0.3)
            .conditions(&Wing {
                span: 1.2,
                chord: 0.2,
            })
            .unwrap();
        assert!((large.required_cl - 0.65).abs() < 0.005);
        assert!((large.reynolds - 75_500.0).abs() < 100.0);
        assert!(large.required_cl < small.required_cl && large.reynolds > small.reynolds);
    }

    #[test]
    fn mach_uses_the_local_speed_of_sound() {
        // 250 m/s at 10 000 m, where the 1976 table gives a = 299.53 m/s.
        let airliner = Flight {
            speed: 250.0,
            altitude: 10_000.0,
            temperature_offset: 0.0,
            mass: 60_000.0,
        };
        let wing = Wing {
            span: 34.0,
            chord: 3.6,
        };
        let c = airliner.conditions(&wing).unwrap();
        assert!((c.mach - 250.0 / 299.53).abs() < 1e-4);
    }

    #[test]
    fn sizing_round_trip() {
        let flight = slow_flyer(0.5);
        for constraint in [WingConstraint::Span(1.5), WingConstraint::AspectRatio(8.0)] {
            let wing = flight.size_wing(0.6, &constraint).unwrap();
            let c = flight.conditions(&wing).unwrap();
            assert!((c.required_cl - 0.6).abs() < 1e-12, "{constraint:?}");
        }
    }

    #[test]
    fn rejects_invalid_flights() {
        let wing = Wing {
            span: 1.0,
            chord: 0.2,
        };
        for flight in [
            Flight {
                speed: 0.0,
                ..slow_flyer(1.0)
            },
            Flight {
                speed: f64::NAN,
                ..slow_flyer(1.0)
            },
            slow_flyer(-1.0),
            Flight {
                altitude: 30_000.0,
                ..slow_flyer(1.0)
            },
        ] {
            assert!(flight.conditions(&wing).is_err(), "{flight:?}");
        }
        assert!(
            slow_flyer(1.0)
                .size_wing(0.0, &WingConstraint::Span(1.0))
                .is_err()
        );
    }

    #[test]
    fn serde_round_trip_and_default_offset() {
        let flight: Flight =
            serde_json::from_str(r#"{"speed": 5.0, "altitude": 100.0, "mass": 0.3}"#).unwrap();
        assert_eq!(flight.temperature_offset, 0.0);
        let json = serde_json::to_string(&flight).unwrap();
        assert_eq!(serde_json::from_str::<Flight>(&json).unwrap(), flight);
    }
}
