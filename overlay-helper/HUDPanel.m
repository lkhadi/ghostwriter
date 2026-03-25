//
//  HUDPanel.m
//  GhostWriter Overlay Helper
//

#import "HUDPanel.h"
#import <WebKit/WebKit.h>
#import <CoreGraphics/CoreGraphics.h>

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
    NSRect frame = NSMakeRect(0, 0, 220, 60);
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
        self.ignoresMouseEvents = NO;
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

    [self orderFrontRegardless];
    [self makeKeyAndOrderFront:nil];
}

- (void)centerNearBottom {
    NSRect screenFrame = [[NSScreen mainScreen] visibleFrame];
    NSLog(@"Screen visible frame: %.0f x %.0f, size: %.0f x %.0f",
             screenFrame.origin.x, screenFrame.origin.y,
             screenFrame.size.width, screenFrame.size.height);

    CGFloat overlayWidth = 220.0;
    CGFloat overlayHeight = 60.0;

    // Center horizontally
    CGFloat x = (screenFrame.size.width - overlayWidth) / 2.0 + screenFrame.origin.x;

    // Position near bottom (100px from bottom edge)
    CGFloat y = screenFrame.origin.y + screenFrame.size.height - overlayHeight - 100.0;

    NSLog(@"Calculated center-bottom position: x: %.0f, y: %.0f", x, y);

    [self setFrameOrigin:NSMakePoint(x, y)];

    // Verify final position
    NSRect finalFrame = self.frame;
    NSLog(@"Final frame after center: %.0f x %.0f, size: %.0f x %.0f",
             finalFrame.origin.x, finalFrame.origin.y,
             finalFrame.size.width, finalFrame.size.height);
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
