/**
 * PCM Audio Processor Worklet
 * Handles real-time buffering and playback of 24kHz Mono Float32 samples.
 */

// Define necessary types for the worklet environment
interface AudioWorkletProcessor {
    readonly port: MessagePort;
    process(inputs: Float32Array[][], outputs: Float32Array[][], parameters: Record<string, Float32Array>): boolean;
}

declare var AudioWorkletProcessor: {
    prototype: AudioWorkletProcessor;
    new(options?: any): AudioWorkletProcessor;
};

declare function registerProcessor(name: string, processorClass: any): void;
declare var currentFrame: number;

class PCMProcessor extends AudioWorkletProcessor {
    private buffer: number[] = [];
    private hasStarted = false;
    private isBuffering = false;
    private startThreshold: number;
    private resumeThreshold: number;

    constructor(options: any) {
        super();

        // Parse options or defaults
        const startSec = (options.processorOptions && options.processorOptions.startSec) || 3.0;

        // 24kHz sample rate
        this.startThreshold = 24000 * startSec;

        // If we run dry, wait for 0.5s of data before resuming
        this.resumeThreshold = 24000 * 0.5;

        this.port.onmessage = (e) => {
            const samples = e.data;
            for (let i = 0; i < samples.length; i++) {
                this.buffer.push(samples[i]);
            }
        };
    }

    process(_inputs: Float32Array[][], outputs: Float32Array[][]) {
        const output = outputs[0];
        const channel = output[0];

        // Report buffer size periodically (approx every 100ms)
        if (currentFrame % 20 === 0) {
            this.port.postMessage({ type: 'buffer_pga', length: this.buffer.length });
        }

        // 1. Initial buffering state
        if (!this.hasStarted) {
            if (this.buffer.length < this.startThreshold) {
                return true; // Still buffering
            }
            this.hasStarted = true;
            this.isBuffering = false;
            this.port.postMessage({ type: 'state', state: 'playing' });
        }

        // 2. Re-buffering state (underrun recovery)
        if (this.isBuffering) {
            if (this.buffer.length < this.resumeThreshold) {
                // Output silence while re-buffering
                this.fillSilence(channel);
                return true;
            }
            // Resume playback
            this.isBuffering = false;
            this.port.postMessage({ type: 'state', state: 'playing' });
        }

        // 3. Playback state
        const samplesToCopy = Math.min(channel.length, this.buffer.length);

        // Copy available samples
        for (let i = 0; i < samplesToCopy; i++) {
            channel[i] = this.buffer[i];
        }

        // Remove consumed samples
        if (samplesToCopy > 0) {
            this.buffer.splice(0, samplesToCopy);
        }

        // Check for underrun
        if (samplesToCopy < channel.length) {
            // We ran out of data!
            // Fill the rest with silence
            for (let i = samplesToCopy; i < channel.length; i++) {
                channel[i] = 0;
            }

            // Enter re-buffering state
            this.isBuffering = true;
            this.port.postMessage({ type: 'state', state: 'buffering' });
        }

        return true;
    }

    private fillSilence(channel: Float32Array) {
        for (let i = 0; i < channel.length; i++) {
            channel[i] = 0;
        }
    }
}

registerProcessor('pcm-processor', PCMProcessor);
