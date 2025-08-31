import { useEffect, useRef, useState } from 'react';
import { useAtom } from 'jotai';
import { containerAtom } from '../state';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
// WebGL renderer can significantly improve rendering.
// Load optionally in case the environment doesn't support it.
import { WebglAddon } from 'xterm-addon-webgl';
import 'xterm/css/xterm.css';

interface TerminalProps {
    containerName?: string;
}

export default function Terminal({ containerName }: TerminalProps) {
    const ref = useRef<HTMLDivElement>(null);
    const termRef = useRef<XTerm | null>(null);
    const wsRef = useRef<WebSocket | null>(null);
    const [container] = useAtom(containerAtom);
    const [isConnecting, setIsConnecting] = useState(false);
    const [isTouchDevice, setIsTouchDevice] = useState(false);
    const activeContainer = containerName || container;

    const sendKey = (key: string) => {
        const ws = wsRef.current;
        if (ws && ws.readyState === WebSocket.OPEN) {
            ws.send(key);
        }
        termRef.current?.focus();
    };

    useEffect(() => {
        setIsTouchDevice('ontouchstart' in window || navigator.maxTouchPoints > 0);
    }, []);

    useEffect(() => {
        if (!activeContainer || !ref.current) return;
        
        setIsConnecting(true);
        
        // Clean up previous terminal if it exists
        if (termRef.current) {
            termRef.current.dispose();
        }
        if (wsRef.current) {
            wsRef.current.close();
        }

        const term = new XTerm({
            cursorBlink: true,
            fontSize: 14,
            fontFamily: 'Menlo, Monaco, "Courier New", monospace',
            scrollback: 5000,
            drawBoldTextInBrightColors: true,
        });
        
        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        // Try to enable WebGL rendering (falls back automatically if unavailable)
        try {
            const webgl = new WebglAddon();
            term.loadAddon(webgl);
        } catch (_) {
            // Ignore if WebGL addon fails to initialize
        }
        
        termRef.current = term;

        try {
            term.open(ref.current);
            term.focus();
            
            // Wait for the terminal to be fully rendered before fitting
            setTimeout(() => {
                try {
                    fitAddon.fit();
                } catch (err) {
                    console.warn('Failed to fit terminal:', err);
                }
            }, 100);

            const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
            const token =
                new URLSearchParams(window.location.search).get('token') ||
                activeContainer;
            // Forward auto-run params to the server so it can inject them immediately
            const pageParams = new URLSearchParams(window.location.search);
            const run = pageParams.get('run');
            const runB64 = pageParams.get('run_b64');
            const cwd = pageParams.get('cwd');
            const cwdB64 = pageParams.get('cwd_b64');
            const wsParams = new URLSearchParams();
            wsParams.set('token', token);
            if (runB64) wsParams.set('run_b64', runB64);
            else if (run) wsParams.set('run', run);
            if (cwdB64) wsParams.set('cwd_b64', cwdB64);
            else if (cwd) wsParams.set('cwd', cwd);

            const ws = new WebSocket(
                `${protocol}://${window.location.host}/terminal/${activeContainer}?${wsParams.toString()}`
            );
            wsRef.current = ws;

            ws.onopen = () => {
                setIsConnecting(false);
                term.write('Connected to container...\r\n');
            };

            ws.onmessage = (e) => {
                try {
                    term.write(e.data);
                } catch (err) {
                    console.error('Failed to write to terminal:', err);
                }
            };

            ws.onerror = (err) => {
                console.error('WebSocket error:', err);
                setIsConnecting(false);
                term.write('\r\nConnection error. Please try again.\r\n');
            };

            ws.onclose = () => {
                setIsConnecting(false);
                term.write('\r\nConnection closed.\r\n');
            };

            term.onData((data) => {
                if (ws.readyState === WebSocket.OPEN) {
                    ws.send(data);
                }
            });

            // Handle window resize: fit xterm
            const handleResize = () => {
                try {
                    fitAddon.fit();
                } catch (err) {
                    console.warn('Failed to fit terminal on resize:', err);
                }
            };

            window.addEventListener('resize', handleResize);

            return () => {
                window.removeEventListener('resize', handleResize);
                if (ws.readyState === WebSocket.OPEN) {
                    ws.close();
                }
                term.dispose();
            };
        } catch (err) {
            console.error('Failed to initialize terminal:', err);
            setIsConnecting(false);
        }
    }, [activeContainer]);

    if (!activeContainer) {
        return (
            <div className="h-full w-full flex items-center justify-center text-gray-500">
                No container specified
            </div>
        );
    }

    return (
        <div className="h-full w-full relative">
            {isConnecting && (
                <div className="absolute inset-0 flex items-center justify-center bg-black bg-opacity-50 text-white z-10">
                    Connecting to terminal...
                </div>
            )}
            {isTouchDevice && (
                <div className="absolute bottom-2 right-2 flex gap-2 z-20">
                    <button
                        className="px-2 py-1 text-sm bg-gray-700 text-white rounded"
                        onClick={() => sendKey('\u0003')}
                    >
                        Ctrl+C
                    </button>
                    <button
                        className="px-2 py-1 text-sm bg-gray-700 text-white rounded"
                        onClick={() => sendKey('\u001b')}
                    >
                        Esc
                    </button>
                </div>
            )}
            <div ref={ref} className="h-full w-full" />
        </div>
    );
}
