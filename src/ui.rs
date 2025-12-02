use atomic_float::AtomicF32;
use nih_plug::prelude::{util, Editor};
// use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::{self, *};
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use vizia::prelude::*;


use nih_plug::prelude::Param;
use nih_plug::params::internals::ParamPtr;

use crate::{OpenMbcParams, MAX_MBCS};

#[derive(Lens)]
struct UiData {
    params: Arc<OpenMbcParams>,
    peak_meter: Arc<AtomicF32>,
}

#[derive(Debug)]
pub enum ParamChangeEvent {
    BeginSet(ParamPtr),
    EndSet(ParamPtr),
    SetParam(ParamPtr, f32),
}

impl Model for UiData {}

// Makes sense to also define this here, makes it a bit easier to keep track of
pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (640, 480))
}


pub(crate) fn create(
    params: Arc<OpenMbcParams>,
    peak_meter: Arc<AtomicF32>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);

        UiData {
            params: params.clone(),
            peak_meter: peak_meter.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            Label::new(cx, "Open Multi Band Compressor");

            // widgets::GenericUi::new(cx, Data::params);

            Knob::new(cx, 0.5, UiData::params.map(|param| param.comps[0].attack.value()), false).on_change(|cx, val|{});

            // make_knob(cx,  params.comps[0].gain.as_ptr(), |params| &params.comps[0].gain);
            // make_knob(cx,  params.comps[0].attack.as_ptr(), |params| &params.comps[0].attack);
        });

        ResizeHandle::new(cx);
    })
}
