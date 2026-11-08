//! Mac'te dört parmak tıklaması → kısayol çarkı (OZELLIKLER.md §3.5).
//!
//! Webview trackpad dokunuş sayısını göremez; yerel bir NSEvent izleyicisi jest olaylarından anlık dokunuş sayısını
//! tutar, sol tık geldiğinde 4+ parmak varsa geri çağırır. Yalnızca uygulama öndeyken (yerel izleyici) çalışır;
//! başka uygulamaların olaylarını dinlemez.

#[cfg(target_os = "macos")]
pub fn install(on_four_finger_click: impl Fn() + 'static) {
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventType, NSTouchPhase};
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TOUCHES: AtomicUsize = AtomicUsize::new(0);

    let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit izleyiciye geçerli bir olay verir; yalnızca okunur ve olduğu gibi geri döndürülür.
        let ev = unsafe { event.as_ref() };
        let kind = ev.r#type();
        if kind == NSEventType::Gesture {
            // touchesMatchingPhase yalnızca jest olaylarında geçerlidir (fare olaylarında çağrılmaz).
            let touching = ev.touchesMatchingPhase_inView(NSTouchPhase::Touching, None);
            TOUCHES.store(touching.count(), Ordering::Relaxed);
        } else if kind == NSEventType::EndGesture {
            TOUCHES.store(0, Ordering::Relaxed);
        } else if kind == NSEventType::LeftMouseDown && TOUCHES.load(Ordering::Relaxed) >= 4 {
            on_four_finger_click();
        }
        event.as_ptr()
    });
    let mask = NSEventMask::Gesture | NSEventMask::EndGesture | NSEventMask::LeftMouseDown;
    // SAFETY: ana iş parçacığında (Tauri setup) çağrılır; izleyici uygulama boyunca yaşar.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &handler) };
    // İzleyici ve blok uygulama kapanana kadar kalır.
    std::mem::forget(monitor);
    std::mem::forget(handler);
}

#[cfg(not(target_os = "macos"))]
pub fn install(_on_four_finger_click: impl Fn() + 'static) {}
