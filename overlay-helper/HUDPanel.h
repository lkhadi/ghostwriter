//
//  HUDPanel.h
//  GhostWriter Overlay Helper
//

#import <Cocoa/Cocoa.h>

@interface HUDPanel : NSPanel

- (instancetype)initWithContentURL:(NSURL *)contentURL;
- (void)showAtX:(CGFloat)x y:(CGFloat)y;
- (void)hide;
- (void)centerNearBottom;
- (void)setWindowLevel:(NSInteger)level;
- (void)setupSpaceMonitoring;

@end
