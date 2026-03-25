//
//  main.m
//  GhostWriter Overlay Helper
//

#import <Foundation/Foundation.h>
#import <Cocoa/Cocoa.h>
#import "HUDPanel.h"
#import <sys/socket.h>
#import <sys/un.h>

@interface SocketServer : NSObject
@property (nonatomic, strong) HUDPanel *hudPanel;
@property (nonatomic, assign) int serverSocket;
@property (nonatomic, assign) BOOL running;
@end

@implementation SocketServer

- (instancetype)initWithContentURL:(NSURL *)contentURL {
    self = [super init];
    if (self) {
        NSLog(@"SocketServer: Initializing");

        self.hudPanel = [[HUDPanel alloc] initWithContentURL:contentURL];
        [self.hudPanel setupSpaceMonitoring];
        self.running = NO;
        [self setupSocket];
    }
    return self;
}

- (void)setupSocket {
    NSLog(@"SocketServer: Setting up socket");

    NSString *socketPath = @"/tmp/ghostwriter_overlay.sock";

    // Remove existing socket if present
    [[NSFileManager defaultManager] removeItemAtPath:socketPath error:nil];

    // Create Unix domain socket
    self.serverSocket = socket(AF_UNIX, SOCK_STREAM, 0);
    if (self.serverSocket == -1) {
        NSLog(@"ERROR: Failed to create socket: %s", strerror(errno));
        return;
    }

    NSLog(@"SocketServer: Socket created successfully");

    // Configure socket address
    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, [socketPath UTF8String], sizeof(addr.sun_path) - 1);

    // Bind socket
    if (bind(self.serverSocket, (struct sockaddr *)&addr, sizeof(addr)) == -1) {
        NSLog(@"ERROR: Failed to bind socket: %s", strerror(errno));
        close(self.serverSocket);
        return;
    }

    NSLog(@"SocketServer: Socket bound successfully");

    // Listen for connections
    if (listen(self.serverSocket, 1) == -1) {
        NSLog(@"ERROR: Failed to listen on socket: %s", strerror(errno));
        close(self.serverSocket);
        return;
    }

    NSLog(@"SocketServer: Listening on %@", socketPath);
    self.running = YES;
    [self acceptConnection];
}

- (void)start {
    NSLog(@"SocketServer: Start method called (no-op, socket already listening)");
}

- (void)acceptConnection {
    NSLog(@"SocketServer: Waiting for connection...");

    dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^{
        while (self.running) {
            NSLog(@"SocketServer: Waiting for accept...");

            int clientSocket = accept(self.serverSocket, NULL, NULL);
            if (clientSocket == -1) {
                if (self.running) {
                    NSLog(@"ERROR: Failed to accept connection: %s", strerror(errno));
                }
                continue;
            }

            NSLog(@"SocketServer: Client connected");
            [self handleClient:clientSocket];
        }
    });
}

- (void)handleClient:(int)clientSocket {
    NSLog(@"SocketServer: Handling client...");

    char buffer[256];
    ssize_t bytesRead;

    while ((bytesRead = read(clientSocket, buffer, sizeof(buffer) - 1)) > 0) {
        buffer[bytesRead] = '\0';
        NSString *command = [NSString stringWithUTF8String:buffer];
        NSLog(@"SocketServer: Received command: %@", command);
        [self processCommand:command];

        // Send acknowledgment
        const char *response = "OK\n";
        write(clientSocket, response, strlen(response));
    }

    close(clientSocket);
    NSLog(@"SocketServer: Client disconnected");
}

- (void)processCommand:(NSString *)command {
    command = [command stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceAndNewlineCharacterSet]];
    NSLog(@"SocketServer: Processing command: %@", command);

    if ([command hasPrefix:@"SHOW"]) {
        // Parse: SHOW x y
        NSArray *components = [command componentsSeparatedByString:@" "];
        if (components.count >= 3) {
            CGFloat x = [[components objectAtIndex:1] floatValue];
            CGFloat y = [[components objectAtIndex:2] floatValue];

            NSLog(@"SocketServer: Calling showAtX:y: with x=%.0f, y=%.0f", x, y);

            dispatch_async(dispatch_get_main_queue(), ^{
                [self.hudPanel showAtX:x y:y];
            });
        }
    } else if ([command isEqualToString:@"HIDE"]) {
        NSLog(@"SocketServer: Calling hide");

        dispatch_async(dispatch_get_main_queue(), ^{
            [self.hudPanel hide];
        });
    } else if ([command isEqualToString:@"QUIT"]) {
        NSLog(@"SocketServer: Quit command received");
        self.running = NO;
        dispatch_async(dispatch_get_main_queue(), ^{
            [NSApp terminate:nil];
        });
    } else if ([command hasPrefix:@"SET_LEVEL"]) {
        // Parse: SET_LEVEL <level_name>
        NSArray *components = [command componentsSeparatedByString:@" "];
        if (components.count >= 2) {
            NSString *levelName = [components objectAtIndex:1];
            NSInteger newLevel = NSFloatingWindowLevel; // default

            if ([levelName isEqualToString:@"MAIN"]) {
                newLevel = NSMainMenuWindowLevel;
            } else if ([levelName isEqualToString:@"FLOATING"]) {
                newLevel = NSFloatingWindowLevel;
            } else if ([levelName isEqualToString:@"STATUS"]) {
                newLevel = NSStatusWindowLevel;
            }

            dispatch_async(dispatch_get_main_queue(), ^{
                [self.hudPanel setWindowLevel:newLevel];
            });
            NSLog(@"Window level set to: %@", levelName);
        }
    } else {
        NSLog(@"SocketServer: Unknown command: %@", command);
    }
}

- (void)dealloc {
    if (self.serverSocket != -1) {
        close(self.serverSocket);
    }
}

@end

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        NSLog(@"GhostWriterOverlayHelper: Starting...");

        NSApplication *app = [NSApplication sharedApplication];
        app.activationPolicy = NSApplicationActivationPolicyAccessory;

        NSLog(@"GhostWriterOverlayHelper: Activation policy set to accessory");

        // Find HUD.html path
        NSString *hudPath = [[NSBundle mainBundle] pathForResource:@"hud" ofType:@"html"];
        if (!hudPath) {
            NSLog(@"ERROR: hud.html not found");
            return 1;
        }

        NSLog(@"GhostWriterOverlayHelper: HUD.html path: %@", hudPath);

        NSURL *contentURL = [NSURL fileURLWithPath:hudPath];
        SocketServer *server = [[SocketServer alloc] initWithContentURL:contentURL];

        NSLog(@"GhostWriterOverlayHelper: SocketServer initialized");

        [server start];

        NSLog(@"GhostWriterOverlayHelper: Running NSApplication");
        [app run];
    }
    return 0;
}
