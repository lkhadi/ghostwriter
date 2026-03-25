#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>

int main() {
    NSLog(@"=== Checking Actual NSMainMenuWindowLevel Value ===");
    NSLog(@"NSNormalWindowLevel = %ld", (long)NSNormalWindowLevel);
    NSLog(@"NSFloatingWindowLevel = %ld", (long)NSFloatingWindowLevel);
    NSLog(@"NSMainMenuWindowLevel = %ld (EXPECTED 18 for Electron)", (long)NSMainMenuWindowLevel);
    NSLog(@"NSStatusWindowLevel = %ld", (long)NSStatusWindowLevel);
    NSLog(@"NSModalPanelWindowLevel = %ld", (long)NSModalPanelWindowLevel);
    
    NSLog(@"\n=== Test Panel with NSMainMenuWindowLevel ===");
    NSPanel *panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel setFloatingPanel:YES];
    [panel setLevel:NSMainMenuWindowLevel];
    [panel setCollectionBehavior:(NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle)];
    NSLog(@"Panel created:");
    NSLog(@"  - level set to: %ld", (long)[panel level]);
    NSLog(@"  - collectionBehavior: %lu", (unsigned long)[panel collectionBehavior]);
    
    return 0;
}
