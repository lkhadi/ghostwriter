#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>

int main() {
    NSLog(@"=== Testing Different Collection Behaviors ===");
    
    // Test 1: Only 3 critical flags
    NSPanel *panel1 = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel1 setFloatingPanel:YES];
    [panel1 setLevel:NSMainMenuWindowLevel];
    [panel1 setCollectionBehavior:(NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle)];
    NSLog(@"Test 1 (4 flags): Behavior = %lu", (unsigned long)[panel1 collectionBehavior]);
    
    // Test 2: All flags together
    NSPanel *panel2 = [[NSPanel alloc] initWithContentRect:NSMakeRect(220, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel2 setFloatingPanel:YES];
    [panel2 setLevel:NSMainMenuWindowLevel];
    [panel2 setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle];
    NSLog(@"Test 2 (4 flags OR'd): Behavior = %lu", (unsigned long)[panel2 collectionBehavior]);
    
    NSLog(@"Both should be the same!");
    
    return 0;
}
