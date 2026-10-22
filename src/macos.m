#import <Cocoa/Cocoa.h>
#import <objc/message.h>
#import <objc/runtime.h>

typedef void (*FileOpenCallback)(const char *path);
static FileOpenCallback g_file_callback = NULL;
static void (*g_quit_callback)(void) = NULL;
void register_macos_quit_callback(void (*callback)(void)) { g_quit_callback = callback; }

// Height of the tab bar, which doubles as the titlebar (tab_bar::HEIGHT).
static const CGFloat kZeroBarHeight = 38.0;
static char kZeroObserved;

// Gives one view instance a private subclass overriding `selector`, leaving
// every other instance of its class untouched.
static void zero_override(NSView *view, NSString *prefix, SEL selector, IMP imp) {
    Class original = object_getClass(view);
    NSString *current = NSStringFromClass(original);
    if ([current hasPrefix:prefix]) return;
    NSString *name = [prefix stringByAppendingString:current];
    Class subclass = NSClassFromString(name);
    if (!subclass) {
        subclass = objc_allocateClassPair(original, name.UTF8String, 0);
        Method method = class_getInstanceMethod(original, selector);
        class_addMethod(subclass, selector, imp, method_getTypeEncoding(method));
        objc_registerClassPair(subclass);
    }
    object_setClass(view, subclass);
}

// The titlebar overlays the tab bar. Let it keep only its buttons, so clicks
// and drags elsewhere reach the tabs underneath.
static NSView *zero_titlebar_hit_test(NSView *self, SEL cmd, NSPoint point) {
    struct objc_super parent = { self, class_getSuperclass(object_getClass(self)) };
    NSView *hit = ((NSView *(*)(struct objc_super *, SEL, NSPoint))objc_msgSendSuper)(&parent, cmd, point);
    for (NSView *view = hit; view && view != self; view = view.superview) {
        if ([view isKindOfClass:NSButton.class]) return hit;
    }
    return nil;
}

// AppKit would otherwise drag the window from any content in the titlebar
// area; the tab bar starts window drags itself from its empty space.
static BOOL zero_never_moves_window(NSView *self __unused, SEL cmd __unused) {
    return NO;
}

// AppKit lays the traffic lights out for a 28pt titlebar. Grow the titlebar
// container to the tab bar's height and centre the buttons inside it.
static void zero_place_traffic_lights(NSWindow *window) {
    if (window.styleMask & NSWindowStyleMaskFullScreen) return;
    NSButton *close = [window standardWindowButton:NSWindowCloseButton];
    NSView *container = close.superview.superview;
    if (!container) return;
    NSRect frame = container.frame;
    frame.size.height = kZeroBarHeight;
    frame.origin.y = NSHeight(window.frame) - kZeroBarHeight;
    container.frame = frame;
    zero_override(container, @"ZeroPassthrough_", @selector(hitTest:), (IMP)zero_titlebar_hit_test);
    zero_override(window.contentView, @"ZeroPinned_", @selector(mouseDownCanMoveWindow),
                  (IMP)zero_never_moves_window);
    CGFloat x = 14.0;
    for (NSNumber *kind in @[@(NSWindowCloseButton), @(NSWindowMiniaturizeButton), @(NSWindowZoomButton)]) {
        NSButton *button = [window standardWindowButton:(NSWindowButton)kind.integerValue];
        NSRect rect = button.frame;
        rect.origin = NSMakePoint(x, round((kZeroBarHeight - NSHeight(rect)) / 2.0));
        button.frame = rect;
        x += 20.0;
    }
}

void style_macos_titlebar(void) {
    for (NSWindow *window in NSApp.windows) {
        if (!window.contentView || window.styleMask == NSWindowStyleMaskBorderless) continue;

        // #292e39 is the chrome surface shared by the tab bar and status bar.
        // A transparent native titlebar lets it show through unchanged.
        window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        window.titlebarAppearsTransparent = YES;
        window.titleVisibility = NSWindowTitleHidden;
        window.backgroundColor = [NSColor colorWithSRGBRed:0.161
                                                     green:0.180
                                                      blue:0.224
                                                     alpha:1.0];
        zero_place_traffic_lights(window);

        if (objc_getAssociatedObject(window, &kZeroObserved)) continue;
        objc_setAssociatedObject(window, &kZeroObserved, @YES, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
        for (NSNotificationName name in @[NSWindowDidResizeNotification,
                                          NSWindowDidExitFullScreenNotification,
                                          NSWindowDidBecomeKeyNotification]) {
            [center addObserverForName:name
                                object:window
                                 queue:NSOperationQueue.mainQueue
                            usingBlock:^(NSNotification *note) {
                                zero_place_traffic_lights(note.object);
                            }];
        }
    }
}

static NSApplicationTerminateReply zero_application_shouldTerminate(id self __unused, SEL cmd __unused, NSApplication *app __unused) {
    if (g_quit_callback) { g_quit_callback(); return NSTerminateCancel; }
    return NSTerminateNow;
}

@interface ZeroAppleEventHandler : NSObject
- (void)handleOpenDocuments:(NSAppleEventDescriptor *)event withReplyEvent:(NSAppleEventDescriptor *)replyEvent;
- (void)handleGetURL:(NSAppleEventDescriptor *)event withReplyEvent:(NSAppleEventDescriptor *)replyEvent;
- (void)processDescriptor:(NSAppleEventDescriptor *)descriptor;
- (void)processSingleItem:(NSAppleEventDescriptor *)item;
@end

@implementation ZeroAppleEventHandler

- (void)handleOpenDocuments:(NSAppleEventDescriptor *)event withReplyEvent:(NSAppleEventDescriptor *)replyEvent {
    (void)replyEvent;
    NSAppleEventDescriptor *param = [event paramDescriptorForKeyword:keyDirectObject];
    [self processDescriptor:param];
}

- (void)handleGetURL:(NSAppleEventDescriptor *)event withReplyEvent:(NSAppleEventDescriptor *)replyEvent {
    (void)replyEvent;
    NSAppleEventDescriptor *param = [event paramDescriptorForKeyword:keyDirectObject];
    [self processDescriptor:param];
}

- (void)processDescriptor:(NSAppleEventDescriptor *)descriptor {
    if (!descriptor || !g_file_callback) return;

    NSInteger count = [descriptor numberOfItems];
    if (count > 0) {
        for (NSInteger i = 1; i <= count; i++) {
            [self processSingleItem:[descriptor descriptorAtIndex:i]];
        }
    } else {
        [self processSingleItem:descriptor];
    }
}

- (void)processSingleItem:(NSAppleEventDescriptor *)item {
    if (!item || !g_file_callback) return;

    NSURL *url = nil;
    if ([item respondsToSelector:@selector(fileURLValue)]) {
        url = [item fileURLValue];
    }
    if (!url) {
        NSAppleEventDescriptor *coerced = [item coerceToDescriptorType:typeFileURL];
        if (coerced && [coerced respondsToSelector:@selector(fileURLValue)]) {
            url = [coerced fileURLValue];
        }
    }
    if (!url) {
        NSString *str = [item stringValue];
        if (str && [str length] > 0) {
            if ([str hasPrefix:@"file://"]) {
                url = [NSURL URLWithString:str];
            } else {
                url = [NSURL fileURLWithPath:str];
            }
        }
    }

    if (url) {
        NSString *path = [url path];
        if (path && [path length] > 0) {
            g_file_callback([path UTF8String]);
        }
    }
}

@end

static ZeroAppleEventHandler *s_event_handler = nil;

static void zero_application_openURLs(id self __unused, SEL _cmd __unused, NSApplication *app __unused, NSArray<NSURL *> *urls) {
    if (!g_file_callback || !urls) return;
    for (NSURL *url in urls) {
        NSString *path = [url path];
        if (path && [path length] > 0) {
            g_file_callback([path UTF8String]);
        }
    }
}

static BOOL zero_application_openFile(id self __unused, SEL _cmd __unused, NSApplication *app __unused, NSString *filename) {
    if (g_file_callback && filename && [filename length] > 0) {
        g_file_callback([filename UTF8String]);
        return YES;
    }
    return NO;
}

static void zero_install_apple_event_handlers(void) {
    if (!s_event_handler) {
        s_event_handler = [[ZeroAppleEventHandler alloc] init];
    }
    NSAppleEventManager *manager = [NSAppleEventManager sharedAppleEventManager];
    
    // kCoreEventClass ('aevt') = 0x61657674, kAEOpenDocuments ('odoc') = 0x6f646f63
    [manager setEventHandler:s_event_handler
                 andSelector:@selector(handleOpenDocuments:withReplyEvent:)
               forEventClass:kCoreEventClass
                  andEventID:kAEOpenDocuments];

    // kInternetEventClass ('GURL') = 0x4755524c, kAEGetURL ('GURL') = 0x4755524c
    [manager setEventHandler:s_event_handler
                 andSelector:@selector(handleGetURL:withReplyEvent:)
               forEventClass:0x4755524c
                  andEventID:0x4755524c];
}

static void zero_hook_delegate(id delegate) {
    if (!delegate) return;
    Class cls = [delegate class];
    SEL quit = @selector(applicationShouldTerminate:);
    if (!class_addMethod(cls, quit, (IMP)zero_application_shouldTerminate, "q@:@")) {
        Method method = class_getInstanceMethod(cls, quit);
        if (method) method_setImplementation(method, (IMP)zero_application_shouldTerminate);
    }
    
    if (!class_getInstanceMethod(cls, @selector(application:openURLs:))) {
        class_addMethod(cls, @selector(application:openURLs:), (IMP)zero_application_openURLs, "v@:@@");
    }
    
    if (!class_getInstanceMethod(cls, @selector(application:openFile:))) {
        class_addMethod(cls, @selector(application:openFile:), (IMP)zero_application_openFile, "c@:@@");
    }
}

static void (*orig_setDelegate)(id, SEL, id) = NULL;

static void zero_setDelegate(id self, SEL _cmd, id delegate) {
    if (orig_setDelegate) {
        orig_setDelegate(self, _cmd, delegate);
    }
    if (delegate) {
        zero_hook_delegate(delegate);
        zero_install_apple_event_handlers();
    }
}

void register_macos_file_callback(FileOpenCallback cb) {
    g_file_callback = cb;

    @autoreleasepool {
        // Swizzle NSApplication setDelegate: so when winit sets the delegate, we hook it
        Class appCls = [NSApplication class];
        SEL sel = @selector(setDelegate:);
        Method m = class_getInstanceMethod(appCls, sel);
        if (m && !orig_setDelegate) {
            orig_setDelegate = (void (*)(id, SEL, id))method_getImplementation(m);
            method_setImplementation(m, (IMP)zero_setDelegate);
        }

        // Install AppleEvent handlers now as well
        zero_install_apple_event_handlers();

        // If delegate is already set on NSApp, hook it now
        NSApplication *app = [NSApplication sharedApplication];
        id delegate = [app delegate];
        if (delegate) {
            zero_hook_delegate(delegate);
        }
    }
}

// ---------------------------------------------------------------------------
// Menu bar. Rust describes the menus (menu.rs); each item's tag indexes that
// table. Key equivalents are only shown: the keystroke is handed back to the
// window, so the editor's own key handling stays the single source of truth.
// ---------------------------------------------------------------------------

static void (*g_menu_action)(int) = NULL;
static int (*g_menu_state)(int) = NULL;

void register_macos_menu_callbacks(void (*action)(int), int (*state)(int)) {
    g_menu_action = action;
    g_menu_state = state;
}

@interface ZeroMenuTarget : NSObject <NSMenuDelegate>
@end

@implementation ZeroMenuTarget
- (void)perform:(NSMenuItem *)item {
    NSEvent *event = NSApp.currentEvent;
    if (event.type == NSEventTypeKeyDown && event.window) {
        [event.window sendEvent:event];
        return;
    }
    if (g_menu_action) g_menu_action((int)item.tag);
}
- (BOOL)validateMenuItem:(NSMenuItem *)item {
    if (item.action == @selector(perform:) && g_menu_state) {
        int state = g_menu_state((int)item.tag);
        item.state = state == 1 ? NSControlStateValueOn : NSControlStateValueOff;
    }
    return YES;
}
- (void)openRecent:(NSMenuItem *)item {
    NSURL *url = item.representedObject;
    if (g_file_callback && url.path) g_file_callback(url.path.fileSystemRepresentation);
}
// Rebuilt on every open so it always matches the system's recent list.
- (void)menuNeedsUpdate:(NSMenu *)menu {
    [menu removeAllItems];
    NSDocumentController *documents = NSDocumentController.sharedDocumentController;
    NSArray<NSURL *> *urls = documents.recentDocumentURLs;
    for (NSURL *url in urls) {
        NSMenuItem *item = [menu addItemWithTitle:url.lastPathComponent
                                           action:@selector(openRecent:)
                                    keyEquivalent:@""];
        item.target = self;
        item.representedObject = url;
        item.toolTip = url.path;
        NSImage *icon = [NSWorkspace.sharedWorkspace iconForFile:url.path];
        icon.size = NSMakeSize(16, 16);
        item.image = icon;
    }
    if (urls.count > 0) [menu addItem:NSMenuItem.separatorItem];
    NSMenuItem *clear = [menu addItemWithTitle:@"Clear Menu"
                                        action:@selector(clearRecentDocuments:)
                                 keyEquivalent:@""];
    clear.target = documents;
}
@end

static ZeroMenuTarget *g_menu_target = nil;
static NSMutableArray<NSMenu *> *g_menus = nil;

static NSString *zero_key(const char *key) {
    if (strcmp(key, "up") == 0) return [NSString stringWithFormat:@"%C", (unichar)NSUpArrowFunctionKey];
    if (strcmp(key, "down") == 0) return [NSString stringWithFormat:@"%C", (unichar)NSDownArrowFunctionKey];
    return @(key);
}

static NSEventModifierFlags zero_modifiers(unsigned mods) {
    NSEventModifierFlags flags = 0;
    if (mods & 1) flags |= NSEventModifierFlagCommand;
    if (mods & 2) flags |= NSEventModifierFlagShift;
    if (mods & 4) flags |= NSEventModifierFlagOption;
    if (mods & 8) flags |= NSEventModifierFlagControl;
    return flags;
}

void zero_menu_begin(const char *title) {
    if (!g_menus) g_menus = [NSMutableArray array];
    if (!g_menu_target) g_menu_target = [[ZeroMenuTarget alloc] init];
    [g_menus addObject:[[NSMenu alloc] initWithTitle:@(title)]];
}

void zero_menu_item(const char *title, const char *key, unsigned mods, int tag) {
    NSMenuItem *item = [g_menus.lastObject addItemWithTitle:@(title)
                                                     action:@selector(perform:)
                                              keyEquivalent:zero_key(key)];
    item.keyEquivalentModifierMask = zero_modifiers(mods);
    item.target = g_menu_target;
    item.tag = tag;
}

void zero_menu_separator(void) {
    [g_menus.lastObject addItem:NSMenuItem.separatorItem];
}

// Items AppKit already knows how to perform.
void zero_menu_native(const char *name) {
    NSMenu *menu = g_menus.lastObject;
    NSString *title = @(name);
    if ([title isEqualToString:@"Open Recent"]) {
        NSMenu *recent = [[NSMenu alloc] initWithTitle:title];
        recent.delegate = g_menu_target;
        [menu addItemWithTitle:title action:NULL keyEquivalent:@""].submenu = recent;
    } else if ([title isEqualToString:@"Enter Full Screen"]) {
        NSMenuItem *item = [menu addItemWithTitle:title action:@selector(toggleFullScreen:) keyEquivalent:@"f"];
        item.keyEquivalentModifierMask = NSEventModifierFlagCommand | NSEventModifierFlagControl;
    } else if ([title isEqualToString:@"Minimize"]) {
        [menu addItemWithTitle:title action:@selector(performMiniaturize:) keyEquivalent:@"m"];
    } else if ([title isEqualToString:@"Zoom"]) {
        [menu addItemWithTitle:title action:@selector(performZoom:) keyEquivalent:@""];
    } else if ([title isEqualToString:@"Bring All to Front"]) {
        [menu addItemWithTitle:title action:@selector(arrangeInFront:) keyEquivalent:@""].target = NSApp;
    }
}

// Keeps the application menu AppKit's loader created and replaces the rest.
void zero_menu_install(void) {
    NSMenu *bar = NSApp.mainMenu;
    if (!bar) {
        bar = [[NSMenu alloc] init];
        NSApp.mainMenu = bar;
    }
    while (bar.numberOfItems > 1) [bar removeItemAtIndex:bar.numberOfItems - 1];
    for (NSMenu *menu in g_menus) {
        NSMenuItem *holder = [bar addItemWithTitle:menu.title action:NULL keyEquivalent:@""];
        holder.submenu = menu;
        if ([menu.title isEqualToString:@"Window"]) NSApp.windowsMenu = menu;
    }
    [g_menus removeAllObjects];
}

void zero_note_recent(const char *path) {
    NSURL *url = [NSURL fileURLWithPath:@(path)];
    [NSDocumentController.sharedDocumentController noteNewRecentDocumentURL:url];
}
