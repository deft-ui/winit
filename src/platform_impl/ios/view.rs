#![allow(clippy::unnecessary_cast)]
use std::cell::{Cell, RefCell};
use libc::unsetenv;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{class, declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2::ffi::{NSInteger, NSUInteger};
use objc2_foundation::{CGFloat, CGPoint, CGRect, MainThreadMarker, NSArray, NSAttributedStringKey, NSComparisonResult, NSDictionary, NSMutableString, NSObject, NSRange, NSSet, NSString};
use objc2_ui_kit::{NSWritingDirection, UICoordinateSpace, UIEvent, UIForceTouchCapability, UIGestureRecognizer, UIGestureRecognizerDelegate, UIGestureRecognizerState, UIKeyInput, UIPanGestureRecognizer, UIPinchGestureRecognizer, UIResponder, UIRotationGestureRecognizer, UITapGestureRecognizer, UITextInput, UITextInputDelegate, UITextInputStringTokenizer, UITextInputTokenizer, UITextInputTraits, UITextLayoutDirection, UITextPosition, UITextRange, UITextSelectionRect, UITextStorageDirection, UITouch, UITouchPhase, UITouchType, UITraitEnvironment, UIView};

use super::app_state::{self, EventWrapper};
use super::window::WinitUIWindow;
use crate::dpi::PhysicalPosition;
use crate::event::{ElementState, Event, Force, Ime, KeyEvent, Touch, TouchPhase, WindowEvent};
use crate::keyboard::{Key, KeyCode, KeyLocation, NamedKey, NativeKeyCode, PhysicalKey};
use crate::platform_impl::platform::DEVICE_ID;
use crate::platform_impl::KeyEventExtra;
use crate::window::{WindowAttributes, WindowId as RootWindowId};

const LOCATION_NOT_FOUND: NSUInteger = NSUInteger::MAX;

#[derive(Clone)]
pub struct TextPositionState {
    offset: i32,
}

#[derive(Clone)]
pub struct TextRangeState {
    range: NSRange,
}

declare_class!(
    pub struct CustomTextPosition;
     unsafe impl ClassType for CustomTextPosition {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "CustomTextPosition";
    }

    impl DeclaredClass for CustomTextPosition {
        type Ivars = TextPositionState;
    }

    unsafe impl CustomTextPosition {
        #[method_id(initWithOffset:)]
        fn init_with_offset(this: Allocated<Self>, offset: i32) -> Option<Retained<Self>> {
            let this = this.set_ivars(TextPositionState {
                offset,
            });
            unsafe { msg_send_id![super(this), init] }
        }

        #[method(offset)]
        fn __get_offset(&self) -> i32 {
            self.ivars().offset
        }
    }
);

declare_class!(
    pub struct CustomTextRange;
    unsafe impl ClassType for CustomTextRange {
        type Super = UITextRange;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CustomTextRange";
    }
    impl DeclaredClass for CustomTextRange {
        type Ivars = TextRangeState;
    }
     unsafe impl CustomTextRange {
        #[method_id(initWithRange:)]
        fn init_with_range(this: Allocated<Self>, range: NSRange) -> Option<Retained<Self>> {
            let this = this.set_ivars(TextRangeState {
                range,
            });
            unsafe { msg_send_id![super(this), init] }
        }

        #[method(isEmpty)]
        fn is_empty(&self) -> bool {
            self.ivars().range.length == 0
        }

        #[method_id(start)]
        fn __get_start(&self) -> Retained<CustomTextPosition> {
            CustomTextPosition::from_offset(self.ivars().range.location as i32)
        }

        #[method_id(end)]
        fn __get_end(&self) -> Retained<CustomTextPosition> {
            CustomTextPosition::from_offset(self.ivars().range.location as i32 + self.ivars().range.length as i32)
        }

    }
);

impl CustomTextRange {

    pub fn from_range(range: NSRange) -> Retained<Self> {
        let obj = unsafe { msg_send_id![CustomTextRange::class(), alloc] };
        let obj: Retained<CustomTextRange> = unsafe { msg_send_id![obj, initWithRange: range]};
        obj
    }

    pub fn from_start_len(start: i32, len: i32) -> Retained<Self> {
        let range = NSRange::new(start as usize, len as usize);
        Self::from_range(range)
    }
}

impl CustomTextPosition {
    pub fn from_offset(offset: i32) -> Retained<Self> {
        let obj = unsafe { msg_send_id![CustomTextPosition::class(), alloc] };
        let obj: Retained<CustomTextPosition> = unsafe { msg_send_id![obj, initWithOffset: offset] };
        obj
    }
}

pub struct WinitViewState {
    pinch_gesture_recognizer: RefCell<Option<Retained<UIPinchGestureRecognizer>>>,
    doubletap_gesture_recognizer: RefCell<Option<Retained<UITapGestureRecognizer>>>,
    rotation_gesture_recognizer: RefCell<Option<Retained<UIRotationGestureRecognizer>>>,
    pan_gesture_recognizer: RefCell<Option<Retained<UIPanGestureRecognizer>>>,

    tkz: RefCell<Option<Retained<UITextInputStringTokenizer>>>,

    // for iOS delta references the start of the Gesture
    rotation_last_delta: Cell<CGFloat>,
    pinch_last_delta: Cell<CGFloat>,
    pan_last_delta: Cell<CGPoint>,

    text: RefCell<Retained<NSMutableString>>,
    selected_text_range: RefCell<NSRange>,
    marked_text_range: RefCell<NSRange>,
}

declare_class!(
    pub(crate) struct WinitView;

    unsafe impl ClassType for WinitView {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitUIView";
    }

    impl DeclaredClass for WinitView {
        type Ivars = WinitViewState;
    }

    unsafe impl WinitView {
        #[method(drawRect:)]
        fn draw_rect(&self, rect: CGRect) {
            let mtm = MainThreadMarker::new().unwrap();
            let window = self.window().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::RedrawRequested,
                }),
            );
            let _: () = unsafe { msg_send![super(self), drawRect: rect] };
        }

        #[method(layoutSubviews)]
        fn layout_subviews(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };

            let window = self.window().unwrap();
            let window_bounds = window.bounds();
            let screen = window.screen();
            let screen_space = screen.coordinateSpace();
            let screen_frame = self.convertRect_toCoordinateSpace(window_bounds, &screen_space);
            let scale_factor = screen.scale();
            let size = crate::dpi::LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            }
            .to_physical(scale_factor as f64);

            // If the app is started in landscape, the view frame and window bounds can be mismatched.
            // The view frame will be in portrait and the window bounds in landscape. So apply the
            // window bounds to the view frame to make it consistent.
            let view_frame = self.frame();
            if view_frame != window_bounds {
                self.setFrame(window_bounds);
            }

            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::Resized(size),
                }),
            );
        }

        #[method(setContentScaleFactor:)]
        fn set_content_scale_factor(&self, untrusted_scale_factor: CGFloat) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () =
                unsafe { msg_send![super(self), setContentScaleFactor: untrusted_scale_factor] };

            // `window` is null when `setContentScaleFactor` is invoked prior to `[UIWindow
            // makeKeyAndVisible]` at window creation time (either manually or internally by
            // UIKit when the `UIView` is first created), in which case we send no events here
            let window = match self.window() {
                Some(window) => window,
                None => return,
            };
            // `setContentScaleFactor` may be called with a value of 0, which means "reset the
            // content scale factor to a device-specific default value", so we can't use the
            // parameter here. We can query the actual factor using the getter
            let scale_factor = self.contentScaleFactor();
            assert!(
                !scale_factor.is_nan()
                    && scale_factor.is_finite()
                    && scale_factor.is_sign_positive()
                    && scale_factor > 0.0,
                "invalid scale_factor set on UIView",
            );
            let scale_factor = scale_factor as f64;
            let bounds = self.bounds();
            let screen = window.screen();
            let screen_space = screen.coordinateSpace();
            let screen_frame = self.convertRect_toCoordinateSpace(bounds, &screen_space);
            let size = crate::dpi::LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            };
            let window_id = RootWindowId(window.id());
            app_state::handle_nonuser_events(
                mtm,
                std::iter::once(EventWrapper::ScaleFactorChanged(
                    app_state::ScaleFactorChanged {
                        window,
                        scale_factor,
                        suggested_size: size.to_physical(scale_factor),
                    },
                ))
                .chain(std::iter::once(EventWrapper::StaticEvent(
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::Resized(size.to_physical(scale_factor)),
                    },
                ))),
            );
        }

        #[method(touchesBegan:withEvent:)]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesMoved:withEvent:)]
        fn touches_moved(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesEnded:withEvent:)]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesCancelled:withEvent:)]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(pinchGesture:)]
        fn pinch_gesture(&self, recognizer: &UIPinchGestureRecognizer) {
            let window = self.window().unwrap();

            let (phase, delta) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().pinch_last_delta.set(recognizer.scale());
                    (TouchPhase::Started, 0.0)
                }
                UIGestureRecognizerState::Changed => {
                    let last_scale: f64 = self.ivars().pinch_last_delta.replace(recognizer.scale());
                    (TouchPhase::Moved, recognizer.scale() - last_scale)
                }
                UIGestureRecognizerState::Ended => {
                    let last_scale: f64 = self.ivars().pinch_last_delta.replace(0.0);
                    (TouchPhase::Moved, recognizer.scale() - last_scale)
                }
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    self.ivars().rotation_last_delta.set(0.0);
                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -recognizer.scale())
                }
                state => panic!("unexpected recognizer state: {:?}", state),
            };

            let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::PinchGesture {
                    device_id: DEVICE_ID,
                    delta: delta as f64,
                    phase,
                },
            });

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[method(doubleTapGesture:)]
        fn double_tap_gesture(&self, recognizer: &UITapGestureRecognizer) {
            let window = self.window().unwrap();

            if recognizer.state() == UIGestureRecognizerState::Ended {
                let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::DoubleTapGesture {
                        device_id: DEVICE_ID,
                    },
                });

                let mtm = MainThreadMarker::new().unwrap();
                app_state::handle_nonuser_event(mtm, gesture_event);
            }
        }

        #[method(rotationGesture:)]
        fn rotation_gesture(&self, recognizer: &UIRotationGestureRecognizer) {
            let window = self.window().unwrap();

            let (phase, delta) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().rotation_last_delta.set(0.0);

                    (TouchPhase::Started, 0.0)
                }
                UIGestureRecognizerState::Changed => {
                    let last_rotation = self.ivars().rotation_last_delta.replace(recognizer.rotation());

                    (TouchPhase::Moved, recognizer.rotation() - last_rotation)
                }
                UIGestureRecognizerState::Ended => {
                    let last_rotation = self.ivars().rotation_last_delta.replace(0.0);

                    (TouchPhase::Ended, recognizer.rotation() - last_rotation)
                }
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    self.ivars().rotation_last_delta.set(0.0);

                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -recognizer.rotation())
                }
                state => panic!("unexpected recognizer state: {:?}", state),
            };

            // Make delta negative to match macos, convert to degrees
            let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::RotationGesture {
                    device_id: DEVICE_ID,
                    delta: -delta.to_degrees() as _,
                    phase,
                },
            });

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[method(panGesture:)]
        fn pan_gesture(&self, recognizer: &UIPanGestureRecognizer) {
            let window = self.window().unwrap();

            let translation = recognizer.translationInView(Some(self));

            let (phase, dx, dy) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().pan_last_delta.set(translation);

                    (TouchPhase::Started, 0.0, 0.0)
                }
                UIGestureRecognizerState::Changed => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(translation);

                    let dx = translation.x - last_pan.x;
                    let dy = translation.y - last_pan.y;

                    (TouchPhase::Moved, dx, dy)
                }
                UIGestureRecognizerState::Ended => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(CGPoint{x:0.0, y:0.0});

                    let dx = translation.x - last_pan.x;
                    let dy = translation.y - last_pan.y;

                    (TouchPhase::Ended, dx, dy)
                }
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(CGPoint{x:0.0, y:0.0});

                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -last_pan.x, -last_pan.y)
                }
                state => panic!("unexpected recognizer state: {:?}", state),
            };


            let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::PanGesture {
                    device_id: DEVICE_ID,
                    delta: PhysicalPosition::new(dx as _, dy as _),
                    phase,
                },
            });

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[method(canBecomeFirstResponder)]
        fn can_become_first_responder(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for WinitView {}

    unsafe impl UIGestureRecognizerDelegate for WinitView {
        #[method(gestureRecognizer:shouldRecognizeSimultaneouslyWithGestureRecognizer:)]
        fn should_recognize_simultaneously(&self, _gesture_recognizer: &UIGestureRecognizer, _other_gesture_recognizer: &UIGestureRecognizer) -> bool {
            true
        }
    }

    unsafe impl UITextInputTraits for WinitView {
    }

    unsafe impl UIKeyInput for WinitView {
        #[method(hasText)]
        fn has_text(&self) -> bool {
            true
        }

        #[method(insertText:)]
        fn insert_text(&self, text: &NSString) {
            self.handle_insert_text(text)
        }

        #[method(deleteBackward)]
        fn delete_backward(&self) {
            self.handle_delete_backward()
        }
    }

    unsafe impl UITextInput for WinitView {
        #[method_id(textInRange:)]
        unsafe fn textInRange(&self, range: &CustomTextRange) -> Option<Retained<NSString>> {
            unsafe {
                Some(self.ivars().text.borrow().substringWithRange(range.ivars().range))
            }
        }

        #[method(replaceRange:withText:)]
        unsafe fn replaceRange_withText(&self, indexed_range: &CustomTextRange, text: &NSString) {
            let mut selected_ns_range = self.ivars().selected_text_range.borrow().clone();
            if indexed_range.ivars().range.location + indexed_range.ivars().range.length <= selected_ns_range.location {
                selected_ns_range.location -= indexed_range.ivars().range.length - text.length();
            }
            unsafe {
                self.ivars().text.borrow_mut().replaceCharactersInRange_withString(indexed_range.ivars().range, text);
            }
            self.ivars().selected_text_range.replace(selected_ns_range);
        }

        #[method_id(selectedTextRange)]
        unsafe fn selectedTextRange(&self) -> Option<Retained<CustomTextRange>> {
            Some(CustomTextRange::from_range(self.ivars().selected_text_range.borrow().clone()))
        }

        #[method(setSelectedTextRange:)]
        unsafe fn setSelectedTextRange(&self, selected_text_range: Option<&CustomTextRange>) {
            *self.ivars().selected_text_range.borrow_mut() = selected_text_range.unwrap().ivars().range;
        }

        #[method_id(markedTextRange)]
        unsafe fn markedTextRange(&self) -> Option<Retained<CustomTextRange>> {
            let range = self.ivars().marked_text_range.borrow().clone();
            if range.length == 0 {
                None
            } else {
                Some(CustomTextRange::from_range(range))
            }
        }

        #[method_id(markedTextStyle)]
        unsafe fn markedTextStyle(
            &self,
        ) -> Option<Retained<NSDictionary<NSAttributedStringKey, AnyObject>>> {
            None
        }

        #[method(setMarkedTextStyle:)]
        unsafe fn setMarkedTextStyle(
            &self,
            marked_text_style: Option<&NSDictionary<NSAttributedStringKey, AnyObject>>,
        ) {

        }

        #[method(setMarkedText:selectedRange:)]
        unsafe fn setMarkedText_selectedRange(
            &self,
            marked_text: Option<&NSString>,
            selected_range: NSRange,
        ) {
            let empty_str = NSString::new();
            let marked_text = marked_text.unwrap_or(&empty_str);
            let mut marked_text_range = self.ivars().marked_text_range.borrow().clone();
            let selected_ns_range = self.ivars().selected_text_range.borrow().clone();

            if marked_text_range.location != LOCATION_NOT_FOUND {
                self.replace_range_with_text(marked_text_range, marked_text);
                marked_text_range.length = marked_text.length();
            } else if selected_ns_range.length > 0 {
                self.replace_range_with_text(selected_ns_range, marked_text);
                marked_text_range.location = selected_ns_range.location;
                marked_text_range.length = marked_text.length();
            } else {
                let mut my_text = self.ivars().text.borrow_mut();
                unsafe {
                    my_text.insertString_atIndex(marked_text, selected_ns_range.location);
                }
                marked_text_range.location = selected_ns_range.location;
                marked_text_range.length = marked_text.length();
            }
            self.ivars().marked_text_range.replace(marked_text_range);
            self.ivars().selected_text_range.replace(
                NSRange::new(selected_range.location + marked_text_range.location, selected_range.length)
            );

            let offset  = if marked_text.is_empty() {
                None
            } else {
                let sub_string_a = unsafe { marked_text.substringToIndex(selected_range.location) };
                let sub_string_b = unsafe { marked_text.substringToIndex(selected_range.end()) };
                let lowerbound_utf8 = sub_string_a.len();
                let upperbound_utf8 = sub_string_b.len();
                Some((lowerbound_utf8, upperbound_utf8))
            };
            self.handle_ime(Ime::Preedit(marked_text.to_string(), offset));
        }

        #[method(unmarkText)]
        unsafe fn unmarkText(&self) {
            let mut marked_text_range = self.ivars().marked_text_range.borrow().clone();
            if marked_text_range.location == LOCATION_NOT_FOUND {
                return;
            }
            marked_text_range.location = LOCATION_NOT_FOUND;
            self.ivars().marked_text_range.replace(marked_text_range);
            self.handle_ime(Ime::Preedit(String::new(), None));
        }

        #[method_id(beginningOfDocument)]
        unsafe fn beginningOfDocument(&self) -> Retained<CustomTextPosition> {
            CustomTextPosition::from_offset(0)
        }

        #[method_id(endOfDocument)]
        unsafe fn endOfDocument(&self) -> Retained<CustomTextPosition> {
            let text = self.ivars().text.borrow();
            let len = text.length() as i32;
            CustomTextPosition::from_offset(len)
        }

        #[method_id(textRangeFromPosition:toPosition:)]
        unsafe fn textRangeFromPosition_toPosition(
            &self,
            from_position: &CustomTextPosition,
            to_position: &CustomTextPosition,
        ) -> Option<Retained<CustomTextRange>> {
            let start = i32::min(from_position.ivars().offset, to_position.ivars().offset);
            let len = i32::abs(to_position.ivars().offset - from_position.ivars().offset);
            Some(CustomTextRange::from_start_len(start, len))
        }

        #[method_id(positionFromPosition:offset:)]
        unsafe fn positionFromPosition_offset(
            &self,
            position: &CustomTextPosition,
            offset: NSInteger,
        ) -> Option<Retained<CustomTextPosition>> {
            let end = position.ivars().offset + offset as i32;
            if end > self.ivars().text.borrow().length() as i32  || end < 0 {
                None
            } else {
                Some(CustomTextPosition::from_offset(end))
            }
        }

        #[method_id(positionFromPosition:inDirection:offset:)]
        unsafe fn positionFromPosition_inDirection_offset(
            &self,
            position: &CustomTextPosition,
            direction: UITextLayoutDirection,
            offset: NSInteger,
        ) -> Option<Retained<CustomTextPosition>> {
            let offset = offset as i32;
            let mut new_position = position.ivars().offset;
            match direction {
                UITextLayoutDirection::Right => {
                    new_position += offset;
                },
                UITextLayoutDirection::Left => {
                    new_position -= offset;
                }
                _ => {},
            }
            if new_position < 0 {
                new_position = 0;
            }
            let text_len = self.ivars().text.borrow().length() as i32;
            if new_position > text_len {
                new_position = text_len;
            }
            Some(CustomTextPosition::from_offset(new_position))
        }

        #[method(comparePosition:toPosition:)]
        unsafe fn comparePosition_toPosition(
            &self,
            position: &CustomTextPosition,
            other: &CustomTextPosition,
        ) -> NSComparisonResult {
            let offset1 = position.ivars().offset;
            let offset2 = other.ivars().offset;
            if offset1 < offset2 {
                NSComparisonResult::Ascending
            } else if offset1 > offset2 {
                NSComparisonResult::Descending
            } else {
                NSComparisonResult::Same
            }
        }

        #[method(offsetFromPosition:toPosition:)]
        unsafe fn offsetFromPosition_toPosition(
            &self,
            from: &CustomTextPosition,
            to_position: &CustomTextPosition,
        ) -> NSInteger {
            let offset1 = from.ivars().offset;
            let offset2 = to_position.ivars().offset;
            (offset2 - offset1) as NSInteger
        }

        #[method_id(inputDelegate)]
        unsafe fn inputDelegate(&self)
            -> Option<Retained<ProtocolObject<dyn UITextInputDelegate>>> {
            None
        }

        #[method(setInputDelegate:)]
        unsafe fn setInputDelegate(
            &self,
            input_delegate: Option<&ProtocolObject<dyn UITextInputDelegate>>,
        ) {

        }

        #[method_id(tokenizer)]
        unsafe fn tokenizer(&self) -> Retained<ProtocolObject<dyn UITextInputTokenizer>> {
            let b = self.ivars().tkz.borrow();
            let t = b.as_ref().unwrap().clone();

            let proto: Retained<ProtocolObject<dyn UITextInputTokenizer>> = ProtocolObject::from_retained(t);
            proto
        }

        #[method_id(positionWithinRange:farthestInDirection:)]
        unsafe fn positionWithinRange_farthestInDirection(
            &self,
            range: &CustomTextRange,
            direction: UITextLayoutDirection,
        ) -> Option<Retained<CustomTextPosition>> {
            match direction {
                UITextLayoutDirection::Up | UITextLayoutDirection::Left => {
                    Some(CustomTextPosition::from_offset(range.ivars().range.location as i32))
                },
                UITextLayoutDirection::Right | UITextLayoutDirection::Down => {
                    Some(CustomTextPosition::from_offset(range.ivars().range.location as i32 + range.ivars().range.length as i32))
                },
                _ => None
            }
        }

        #[method_id(characterRangeByExtendingPosition:inDirection:)]
        unsafe fn characterRangeByExtendingPosition_inDirection(
            &self,
            position: &UITextPosition,
            direction: UITextLayoutDirection,
        ) -> Option<Retained<UITextRange>> {
            None
        }

        #[method(baseWritingDirectionForPosition:inDirection:)]
        unsafe fn baseWritingDirectionForPosition_inDirection(
            &self,
            position: &UITextPosition,
            direction: UITextStorageDirection,
        ) -> NSWritingDirection {
            NSWritingDirection::LeftToRight
        }

        #[method(setBaseWritingDirection:forRange:)]
        unsafe fn setBaseWritingDirection_forRange(
            &self,
            writing_direction: NSWritingDirection,
            range: &UITextRange,
        ) {

        }

        #[method(firstRectForRange:)]
        unsafe fn firstRectForRange(&self, range: &UITextRange) -> CGRect {
            CGRect::default()
        }

        #[method(caretRectForPosition:)]
        unsafe fn caretRectForPosition(&self, position: &UITextPosition) -> CGRect {
            CGRect::default()
        }

        #[method_id(selectionRectsForRange:)]
        unsafe fn selectionRectsForRange(
            &self,
            range: &UITextRange,
        ) -> Retained<NSArray<UITextSelectionRect>> {
            NSArray::new()
        }

        #[method_id(closestPositionToPoint:)]
        unsafe fn closestPositionToPoint(&self, point: CGPoint)
            -> Option<Retained<UITextPosition>> {
            None
        }

        #[method_id(closestPositionToPoint:withinRange:)]
        unsafe fn closestPositionToPoint_withinRange(
            &self,
            point: CGPoint,
            range: &UITextRange,
        ) -> Option<Retained<UITextPosition>> {
            None
        }

        #[method_id(characterRangeAtPoint:)]
        unsafe fn characterRangeAtPoint(&self, point: CGPoint) -> Option<Retained<UITextRange>> {
            None
        }

    }

);

impl WinitView {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        window_attributes: &WindowAttributes,
        frame: CGRect,
    ) -> Retained<Self> {
        let text = NSMutableString::new();
        let this = mtm.alloc().set_ivars(WinitViewState {
            pinch_gesture_recognizer: RefCell::new(None),
            doubletap_gesture_recognizer: RefCell::new(None),
            rotation_gesture_recognizer: RefCell::new(None),
            pan_gesture_recognizer: RefCell::new(None),
            text: RefCell::new(text),
            selected_text_range: RefCell::new(NSRange::new(0, 0)),
            marked_text_range: RefCell::new(NSRange::new(LOCATION_NOT_FOUND, 0)),

            rotation_last_delta: Cell::new(0.0),
            pinch_last_delta: Cell::new(0.0),
            pan_last_delta: Cell::new(CGPoint { x: 0.0, y: 0.0 }),
            tkz: RefCell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send_id![super(this), initWithFrame: frame] };

        unsafe {
            let tokenizer = mtm.alloc::<UITextInputStringTokenizer>();
            let tkz = UITextInputStringTokenizer::initWithTextInput(tokenizer, this.as_ref());
            this.ivars().tkz.replace(Some(tkz));
        }

        this.setMultipleTouchEnabled(true);

        if let Some(scale_factor) = window_attributes.platform_specific.scale_factor {
            this.setContentScaleFactor(scale_factor as _);
        }

        this
    }

    fn replace_range_with_text(&self, range: NSRange, text: &NSString) {
        let mut text_store = self.ivars().text.borrow_mut();
        unsafe {
            text_store.replaceCharactersInRange_withString(range, text);
        }
    }

    fn window(&self) -> Option<Retained<WinitUIWindow>> {
        // SAFETY: `WinitView`s are always installed in a `WinitUIWindow`
        (**self).window().map(|window| unsafe { Retained::cast(window) })
    }

    pub(crate) fn recognize_pinch_gesture(&self, should_recognize: bool) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().pinch_gesture_recognizer.borrow().is_none() {
                let pinch = unsafe {
                    UIPinchGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(pinchGesture:)),
                    )
                };
                pinch.setDelegate(Some(ProtocolObject::from_ref(self)));
                self.addGestureRecognizer(&pinch);
                self.ivars().pinch_gesture_recognizer.replace(Some(pinch));
            }
        } else if let Some(recognizer) = self.ivars().pinch_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    pub(crate) fn recognize_pan_gesture(
        &self,
        should_recognize: bool,
        minimum_number_of_touches: u8,
        maximum_number_of_touches: u8,
    ) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().pan_gesture_recognizer.borrow().is_none() {
                let pan = unsafe {
                    UIPanGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(panGesture:)),
                    )
                };
                pan.setDelegate(Some(ProtocolObject::from_ref(self)));
                pan.setMinimumNumberOfTouches(minimum_number_of_touches as _);
                pan.setMaximumNumberOfTouches(maximum_number_of_touches as _);
                self.addGestureRecognizer(&pan);
                self.ivars().pan_gesture_recognizer.replace(Some(pan));
            }
        } else if let Some(recognizer) = self.ivars().pan_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    pub(crate) fn recognize_doubletap_gesture(&self, should_recognize: bool) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().doubletap_gesture_recognizer.borrow().is_none() {
                let tap = unsafe {
                    UITapGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(doubleTapGesture:)),
                    )
                };
                tap.setDelegate(Some(ProtocolObject::from_ref(self)));
                tap.setNumberOfTapsRequired(2);
                tap.setNumberOfTouchesRequired(1);
                self.addGestureRecognizer(&tap);
                self.ivars().doubletap_gesture_recognizer.replace(Some(tap));
            }
        } else if let Some(recognizer) = self.ivars().doubletap_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    pub(crate) fn recognize_rotation_gesture(&self, should_recognize: bool) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().rotation_gesture_recognizer.borrow().is_none() {
                let rotation = unsafe {
                    UIRotationGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(rotationGesture:)),
                    )
                };
                rotation.setDelegate(Some(ProtocolObject::from_ref(self)));
                self.addGestureRecognizer(&rotation);
                self.ivars().rotation_gesture_recognizer.replace(Some(rotation));
            }
        } else if let Some(recognizer) = self.ivars().rotation_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    fn handle_touches(&self, touches: &NSSet<UITouch>) {
        let window = self.window().unwrap();
        let mut touch_events = Vec::new();
        let os_supports_force = app_state::os_capabilities().force_touch;
        for touch in touches {
            let logical_location = touch.locationInView(None);
            let touch_type = touch.r#type();
            let force = if os_supports_force {
                let trait_collection = self.traitCollection();
                let touch_capability = trait_collection.forceTouchCapability();
                // Both the OS _and_ the device need to be checked for force touch support.
                if touch_capability == UIForceTouchCapability::Available
                    || touch_type == UITouchType::Pencil
                {
                    let force = touch.force();
                    let max_possible_force = touch.maximumPossibleForce();
                    let altitude_angle: Option<f64> = if touch_type == UITouchType::Pencil {
                        let angle = touch.altitudeAngle();
                        Some(angle as _)
                    } else {
                        None
                    };
                    Some(Force::Calibrated {
                        force: force as _,
                        max_possible_force: max_possible_force as _,
                        altitude_angle,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let touch_id = touch as *const UITouch as u64;
            let phase = touch.phase();
            let phase = match phase {
                UITouchPhase::Began => TouchPhase::Started,
                UITouchPhase::Moved => TouchPhase::Moved,
                // 2 is UITouchPhase::Stationary and is not expected here
                UITouchPhase::Ended => TouchPhase::Ended,
                UITouchPhase::Cancelled => TouchPhase::Cancelled,
                _ => panic!("unexpected touch phase: {phase:?}"),
            };

            let physical_location = {
                let scale_factor = self.contentScaleFactor();
                PhysicalPosition::from_logical::<(f64, f64), f64>(
                    (logical_location.x as _, logical_location.y as _),
                    scale_factor as f64,
                )
            };
            touch_events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::Touch(Touch {
                    device_id: DEVICE_ID,
                    id: touch_id,
                    location: physical_location,
                    force,
                    phase,
                }),
            }));
        }
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(mtm, touch_events);
    }

    fn handle_insert_text(&self, text: &NSString) {
        let window = self.window().unwrap();
        let window_id = RootWindowId(window.id());
        let mtm = MainThreadMarker::new().unwrap();
        // send individual events for each character
        app_state::handle_nonuser_events(
            mtm,
            text.to_string().chars().flat_map(|c| {
                let text = smol_str::SmolStr::from_iter([c]);
                // Emit both press and release events
                [ElementState::Pressed, ElementState::Released].map(|state| {
                    EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
                            event: KeyEvent {
                                text: if state == ElementState::Pressed {
                                    Some(text.clone())
                                } else {
                                    None
                                },
                                state,
                                location: KeyLocation::Standard,
                                repeat: false,
                                logical_key: Key::Character(text.clone()),
                                physical_key: PhysicalKey::Unidentified(
                                    NativeKeyCode::Unidentified,
                                ),
                                platform_specific: KeyEventExtra {},
                            },
                            is_synthetic: false,
                            device_id: DEVICE_ID,
                        },
                    })
                })
            }),
        );
    }

    fn handle_delete_backward(&self) {
        let window = self.window().unwrap();
        let window_id = RootWindowId(window.id());
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(
            mtm,
            [ElementState::Pressed, ElementState::Released].map(|state| {
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        event: KeyEvent {
                            state,
                            logical_key: Key::Named(NamedKey::Backspace),
                            physical_key: PhysicalKey::Code(KeyCode::Backspace),
                            platform_specific: KeyEventExtra {},
                            repeat: false,
                            location: KeyLocation::Standard,
                            text: None,
                        },
                        is_synthetic: false,
                    },
                })
            }),
        );
    }

    fn handle_ime(&self, ime: Ime) {
        let window = self.window().unwrap();
        let event = EventWrapper::StaticEvent(Event::WindowEvent {
            window_id: RootWindowId(window.id()),
            event: WindowEvent::Ime(ime),
        });
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(mtm, vec![event]);
    }
}
