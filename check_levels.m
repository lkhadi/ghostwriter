#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>

int main() {
    NSLog(@"=== Checking Window Level Constants ===");
    NSLog(@"NSNormalWindowLevel = %ld", (long)NSNormalWindowLevel);
    NSLog(@"NSFloatingWindowLevel = %ld", (long)NSFloatingWindowLevel);
    NSLog(@"NSMainMenuWindowLevel = %ld", (long)NSMainMenuWindowLevel);
    NSLog(@"NSStatusWindowLevel = %ld", (long)NSStatusWindowLevel);
    
    NSLog(@"\n=== Checking Collection Behavior Flags ===");
    NSLog(@"FullScreenAuxiliary = 1 << 0 = %ld", 1L);
    NSLog(@"CanJoinAllSpaces = 1 << 1 = %ld", 2L);
    NSLog(@"Stationary = 1 << 4 = %ld", 16L);
    NSLog(@"IgnoresCycle = 1 << 6 = %ld", 64L);
    
    NSLog(@"\n=== Expected Combined Value ===");
    NSLog(@"1 + 2 + 16 + 64 = %ld", 1L + 2L + 16L + 64L);
    
    NSLog(@"\n=== Current Implementation ===");
    NSPanel *panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel setFloatingPanel:YES];
    [panel setLevel:NSMainMenuWindowLevel];
    [panel setCollectionBehavior:(NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle)];
    NSLog(@"Actual Collection Behavior: %lu", (unsigned long)[panel collectionBehavior]);
    
    return 0;
}
