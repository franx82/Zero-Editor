#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>

typedef void (*FileOpenCallback)(const char *path);
static FileOpenCallback g_file_callback = NULL;
static void (*g_quit_callback)(void) = NULL;
void register_macos_quit_callback(void (*callback)(void)) { g_quit_callback = callback; }

void style_macos_titlebar(void) {
    NSWindow *window = [NSApp mainWindow];
    if (!window) return;

    // Nord1 (#3b4252) is the editor header and tab-bar surface. A transparent
    // native titlebar lets this solid window background show through unchanged.
    window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    window.titlebarAppearsTransparent = YES;
    window.backgroundColor = [NSColor colorWithSRGBRed:0.231
                                                 green:0.259
                                                  blue:0.322
                                                 alpha:1.0];
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
