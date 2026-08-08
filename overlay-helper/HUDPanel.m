//
//  HUDPanel.m
//  GhostWriter Overlay Helper
//

#import "HUDPanel.h"
#import <WebKit/WebKit.h>
#import <CoreGraphics/CoreGraphics.h>

// HUD geometry. Owned here rather than passed in over the socket: this
// process runs on the main thread with direct NSScreen access, including
// visibleFrame.origin, which a cross-process size-only handshake loses.
static const CGFloat kHUDWidth = 220.0;
static const CGFloat kHUDHeight = 60.0;
static const CGFloat kHUDBottomMargin = 100.0;

@interface HUDPanel () <WKNavigationDelegate>
@property (nonatomic, strong) WKWebView *webView;
@end

@implementation HUDPanel

- (void)setupSpaceMonitoring {
    NSLog(@"Setting up space change monitoring");
    [[NSWorkspace sharedWorkspace] addObserver:self
                                      forKeyPath:@"activeSpace"
                                         options:0
                                         context:NULL];
}

- (void)observeValueForKeyPath:(NSString *)keyPath
                      ofObject:(id)object
                        change:(NSDictionary *)change
                       context:(void *)context {
    if ([keyPath isEqualToString:@"activeSpace"]) {
        NSLog(@"Active space changed, repositioning HUD");
        [self centerNearBottom];
    }
}

- (void)dealloc {
    [[NSWorkspace sharedWorkspace] removeObserver:self forKeyPath:@"activeSpace"];
    NSLog(@"Space monitoring removed");
}

- (void)setWindowLevel:(NSInteger)level {
    NSLog(@"Setting window level to: %ld", (long)level);
    self.level = level;
    NSLog(@"New window level applied: %ld", (long)self.level);
}

- (instancetype)initWithContentURL:(NSURL *)contentURL {
    NSRect frame = NSMakeRect(0, 0, kHUDWidth, kHUDHeight);
    self = [super initWithContentRect:frame
                           styleMask:NSWindowStyleMaskBorderless
                             backing:NSBackingStoreBuffered
                               defer:NO];
    if (self) {
        // Critical: Configure as floating panel for fullscreen support
        [self setFloatingPanel:YES];

        // Set window level to appear above fullscreen apps
        // CRITICAL: kCGMainMenuWindowLevel - 1 (level 23) is required for Electron apps (VSCode)
        // based on Stack Overflow #36205834 research
        self.level = kCGMainMenuWindowLevel - 1;

        // CRITICAL: Collection behaviors for fullscreen space support
        // Only set these explicitly - no automatic flags from Cocoa
        self.collectionBehavior = NSWindowCollectionBehaviorFullScreenAuxiliary |
                              NSWindowCollectionBehaviorCanJoinAllSpaces |
                              NSWindowCollectionBehaviorStationary |
                              NSWindowCollectionBehaviorIgnoresCycle;

        // Window properties
        self.backgroundColor = [NSColor clearColor];
        self.opaque = NO;
        self.hasShadow = YES;
        // Passive indicator: never intercept clicks meant for the app below.
        // It cannot become key either, so clicks used to land nowhere at all.
        self.ignoresMouseEvents = YES;
        self.releasedWhenClosed = NO;

        // Hide standard window buttons
        [self standardWindowButton:NSWindowCloseButton].hidden = YES;
        [self standardWindowButton:NSWindowMiniaturizeButton].hidden = YES;
        [self standardWindowButton:NSWindowZoomButton].hidden = YES;

        // Prevent app from activating when showing
        [self setHidesOnDeactivate:NO];

        // Setup WebView
        WKWebViewConfiguration *config = [[WKWebViewConfiguration alloc] init];
        self.webView = [[WKWebView alloc] initWithFrame:frame configuration:config];
        self.webView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        self.webView.navigationDelegate = self;
        [self.webView setValue:@NO forKey:@"drawsBackground"];

         [self.contentView addSubview:self.webView];

         // Load content
         [self.webView loadFileURL:contentURL allowingReadAccessToURL:contentURL.URLByDeletingLastPathComponent];

         // Log window configuration for debugging
        NSLog(@"HUDPanel initialized:");
        NSLog(@"  - Window Level: %ld (should be 18 for Electron apps)", (long)self.level);
        NSLog(@"  - Collection Behavior: %lu", (unsigned long)self.collectionBehavior);
        NSLog(@"  - Floating Panel: %@", self.floatingPanel ? @"YES" : @"NO");
        NSLog(@"  - Can Become Key Window: NO");
     }
     return self;
}

- (void)showAtX:(CGFloat)x y:(CGFloat)y {
    NSLog(@"Positioning HUD at x: %.0f, y: %.0f", x, y);

    // Get screen dimensions
    NSRect screenFrame = [[NSScreen mainScreen] visibleFrame];
    NSLog(@"Screen visible frame: %.0f x %.0f, size: %.0f x %.0f",
             screenFrame.origin.x, screenFrame.origin.y,
             screenFrame.size.width, screenFrame.size.height);

    // Simple approach: just use setFrameOrigin
    [self setFrameOrigin:NSMakePoint(x, y)];

    // Verify final position
    NSRect finalFrame = self.frame;
    NSLog(@"Final frame: %.0f x %.0f, size: %.0f x %.0f",
             finalFrame.origin.x, finalFrame.origin.y,
             finalFrame.size.width, finalFrame.size.height);

    // No makeKeyAndOrderFront: canBecomeKeyWindow returns NO, so it would
    // do nothing beyond what orderFrontRegardless already did.
    [self orderFrontRegardless];
}

- (void)centerNearBottom {
    // visibleFrame uses bottom-left origin coordinates and already excludes
    // the menu bar and Dock. On a multi-display setup its origin is often
    // non-zero or negative, so it must be added rather than assumed to be
    // (0,0). Adding size.height here would place the HUD near the TOP.
    NSRect visible = [[NSScreen mainScreen] visibleFrame];

    CGFloat x = visible.origin.x + (visible.size.width - kHUDWidth) / 2.0;
    CGFloat y = visible.origin.y + kHUDBottomMargin;

    NSLog(@"Centering HUD on visible frame %.0f,%.0f %.0fx%.0f -> %.0f,%.0f",
          visible.origin.x, visible.origin.y,
          visible.size.width, visible.size.height, x, y);

    [self setFrameOrigin:NSMakePoint(x, y)];

    NSRect finalFrame = self.frame;
    NSLog(@"Final frame: %.0f,%.0f size %.0fx%.0f",
          finalFrame.origin.x, finalFrame.origin.y,
          finalFrame.size.width, finalFrame.size.height);
}

- (void)showCentered {
    [self centerNearBottom];
    [self orderFrontRegardless];
}

- (void)hide {
    [self orderOut:nil];
}

#pragma mark - NSWindow overrides

- (BOOL)canBecomeKeyWindow {
    return NO;
}

- (BOOL)canBecomeMainWindow {
    return NO;
}

#pragma mark - WKNavigationDelegate

- (void)webView:(WKWebView *)webView didFinishNavigation:(WKNavigation *)navigation {
    // WebView loaded successfully
}

- (void)webView:(WKWebView *)webView didFailNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    NSLog(@"WebView navigation failed: %@", error.localizedDescription);
}

@end
