#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>

int main() {
    NSLog(@"=== Finding All NSWindowCollectionBehavior Flags ===");
    
    // Test what Cocoa adds automatically
    NSLog(@"\n1. Setting NO flags explicitly:");
    NSPanel *panel1 = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel1 setFloatingPanel:YES];
    [panel1 setLevel:NSMainMenuWindowLevel];
    [panel1 setCollectionBehavior:0];
    NSLog(@"No explicit flags: Behavior = %lu", (unsigned long)[panel1 collectionBehavior]);
    
    NSLog(@"\n2. Setting just FullScreenAuxiliary:");
    NSPanel *panel2 = [[NSPanel alloc] initWithContentRect:NSMakeRect(220, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel2 setFloatingPanel:YES];
    [panel2 setLevel:NSMainMenuWindowLevel];
    [panel2 setCollectionBehavior:NSWindowCollectionBehaviorFullScreenAuxiliary];
    NSLog(@"Just FullScreenAuxiliary: Behavior = %lu", (unsigned long)[panel2 collectionBehavior]);
    
    NSLog(@"\n3. Setting just CanJoinAllSpaces:");
    NSPanel *panel3 = [[NSPanel alloc] initWithContentRect:NSMakeRect(440, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel3 setFloatingPanel:YES];
    [panel3 setLevel:NSMainMenuWindowLevel];
    [panel3 setCollectionBehavior:NSWindowCollectionBehaviorCanJoinAllSpaces];
    NSLog(@"Just CanJoinAllSpaces: Behavior = %lu", (unsigned long)[panel3 collectionBehavior]);
    
    NSLog(@"\n4. Setting floatingPanel=YES:");
    NSPanel *panel4 = [[NSPanel alloc] initWithContentRect:NSMakeRect(660, 0, 200, 200) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:YES];
    [panel4 setFloatingPanel:YES];
    [panel4 setLevel:NSMainMenuWindowLevel];
    [panel4 setCollectionBehavior:0];
    NSLog(@"Before setFloatingPanel: Behavior = %lu", (unsigned long)[panel4 collectionBehavior]);
    [panel4 setFloatingPanel:YES];
    NSLog(@"After setFloatingPanel: Behavior = %lu", (unsigned long)[panel4 collectionBehavior]);
    
    return 0;
}
