use super::wrapper::*;
use std::{
    collections::VecDeque,
    ffi::{c_char, c_void, CStr},
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BoxLayout {
    Horizontal,
    Vertical,
}

impl BoxLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BOX => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BOX => Self::Vertical,
            _ => panic!("Not a BoxLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ButtonLayout {
    Held,
    Checkbox,
}

impl ButtonLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::BUTTON => Self::Held,
            WWidgetDeclType::CHECK_BUTTON => Self::Checkbox,
            _ => panic!("Not a ButtonLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NumericLayout {
    NumEntry,
    HorizontalSlider,
    VerticalSlider,
}

impl NumericLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::NUM_ENTRY => Self::NumEntry,
            WWidgetDeclType::HORIZONTAL_SLIDER => Self::HorizontalSlider,
            WWidgetDeclType::VERTICAL_SLIDER => Self::VerticalSlider,
            _ => panic!("Not a NumericLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BargraphLayout {
    Horizontal,
    Vertical,
}

impl BargraphLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BARGRAPH => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BARGRAPH => Self::Vertical,
            _ => panic!("Not a BargraphLayout"),
        }
    }
}

/// Lifetime 'a corresponds to that of the dsp object
#[derive(Debug)]
pub enum DspWidget<Z> {
    TabGroup {
        label: String,
        inner: Vec<DspWidget<Z>>,
        selected: usize,
    },
    Box {
        layout: BoxLayout,
        label: String,
        inner: Vec<DspWidget<Z>>,
    },
    Button {
        layout: ButtonLayout,
        label: String,
        zone: Z,
    },
    Numeric {
        layout: NumericLayout,
        label: String,
        zone: Z,
        init: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    Bargraph {
        layout: BargraphLayout,
        label: String,
        zone: Z,
        min: f32,
        max: f32,
    },
    // Soundfile {
    //     label: String,
    //     path: PathBuf,
    //     // TODO
    // },
}

impl<Z> DspWidget<Z> {
    fn inner_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            DspWidget::TabGroup { inner, .. } => Some(inner),
            DspWidget::Box { inner, .. } => Some(inner),
            _ => None,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            DspWidget::TabGroup { label, .. } => label,
            DspWidget::Box { label, .. } => label,
            DspWidget::Button { label, .. } => label,
            DspWidget::Numeric { label, .. } => label,
            DspWidget::Bargraph { label, .. } => label,
        }
    }
}

pub struct DspWidgetsBuilder {
    widget_decls: VecDeque<(String, WWidgetDecl)>,
}

/// A memory zone corresponding to some parameter's current value
pub trait Zone {
    fn from_zone_ptr(ptr: *mut f32) -> Self;

    fn cur_value(&self) -> f32;
}

impl<'a> Zone for &'a mut f32 {
    fn from_zone_ptr(ptr: *mut f32) -> Self {
        unsafe { ptr.as_mut() }.unwrap()
    }

    fn cur_value(&self) -> f32 {
        **self
    }
}

impl DspWidgetsBuilder {
    pub fn new() -> Self {
        Self {
            widget_decls: VecDeque::new(),
        }
    }

    pub fn build_widgets<Z: Zone>(mut self, widget_list: &mut Vec<DspWidget<Z>>) {
        self.build_widgets_rec(widget_list);
        assert!(
            self.widget_decls.is_empty(),
            "Some widget declarations haven't been consumed"
        );
    }

    /// To be called _after_ faust's buildUserInterface has finished, ie. after
    /// w_createUIs has finished. 'a is the lifetime of the DSP itself
    fn build_widgets_rec<Z: Zone>(&mut self, cur_level: &mut Vec<DspWidget<Z>>) {
        use WWidgetDeclType as W;
        while let Some((label, decl)) = self.widget_decls.pop_front() {
            let mut widget = match decl.typ {
                W::CLOSE_BOX => return,
                W::TAB_BOX => DspWidget::TabGroup {
                    label,
                    inner: vec![],
                    selected: 0,
                },
                W::HORIZONTAL_BOX | W::VERTICAL_BOX => DspWidget::Box {
                    layout: BoxLayout::from_decl_type(decl.typ),
                    label,
                    inner: vec![],
                },
                W::BUTTON | W::CHECK_BUTTON => DspWidget::Button {
                    layout: ButtonLayout::from_decl_type(decl.typ),
                    label,
                    zone: Zone::from_zone_ptr(decl.zone),
                },
                W::HORIZONTAL_SLIDER | W::VERTICAL_SLIDER | W::NUM_ENTRY => DspWidget::Numeric {
                    layout: NumericLayout::from_decl_type(decl.typ),
                    label,
                    zone: Zone::from_zone_ptr(decl.zone),
                    init: decl.init,
                    min: decl.min,
                    max: decl.max,
                    step: decl.step,
                },
                W::HORIZONTAL_BARGRAPH | W::VERTICAL_BARGRAPH => DspWidget::Bargraph {
                    layout: BargraphLayout::from_decl_type(decl.typ),
                    label,
                    zone: Zone::from_zone_ptr(decl.zone),
                    min: decl.min,
                    max: decl.max,
                },
            };
            if let Some(inner) = widget.inner_mut() {
                // We recurse, so as to add to the newly opened box:
                self.build_widgets_rec(inner);
            }
            cur_level.push(widget);
        }
    }
}

pub(crate) extern "C" fn widget_decl_callback(
    builder_ptr: *mut c_void,
    label_ptr: *const c_char,
    decl: WWidgetDecl,
) {
    let builder = unsafe { (builder_ptr as *mut DspWidgetsBuilder).as_mut() }.unwrap();
    let c_label = unsafe { CStr::from_ptr(label_ptr) };
    let label = match c_label.to_str() {
        Ok("0x00") => "".to_string(),
        Ok(s) => s.to_string(),
        _ => {
            // Label couldn't parse to utf8. We just hash the raw CStr to get
            // some label:
            let mut state = std::hash::DefaultHasher::new();
            c_label.hash(&mut state);
            state.finish().to_string()
        }
    };
    builder.widget_decls.push_back((label, decl));
}
