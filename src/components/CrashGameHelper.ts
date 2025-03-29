import { Ref } from "vue";

export interface Cashout {
    playerId: string;
    amount: number;
    multiplier: number;
    playerName: string;
}

// Canvas and rendering constants
const X_PADDING = 40;
const Y_PADDING = 20;
const RIGHT_PADDING = 40;
const X_AXIS_HEIGHT = 20;

// Map multiplier values to Y positions with proper scaling
function mapMultiplierToY(multiplier: number, height: number, graphHeight: number): number {
    // Base position (1x multiplier)
    const baseY = height - X_AXIS_HEIGHT;

    // Only start increasing scale when above 1.5x
    let maxDisplayMultiplier = 2.4;
    if (multiplier > 1.5) {
        maxDisplayMultiplier = Math.max(2.4, multiplier * 1.2);
    }

    // Much more aggressive adjustment of space allocated to low multipliers
    // Starting with a lower default and reducing more drastically
    let lowRangePercentage = 0.25; // Default 25% for 1x-1.5x (already reduced from 50%)

    // Aggressively reduce space for low range as multiplier increases
    if (multiplier > 3) {
        lowRangePercentage = 0.15; // 15% space for 1x-1.5x
    }
    if (multiplier > 5) {
        lowRangePercentage = 0.1; // 10% space for 1x-1.5x
    }
    if (multiplier > 10) {
        lowRangePercentage = 0.05; // 5% space for 1x-1.5x
    }
    if (multiplier > 20) {
        lowRangePercentage = 0.04; // 4% space
    }
    if (multiplier > 30) {
        lowRangePercentage = 0.02; // Ultra-compressed at just 2% of height
    }

    // For values below or equal to 1.5x, use the aggressively reduced percentage
    if (multiplier <= 1.5) {
        // Linear mapping for values between 1.0-1.5x
        const normalizedPosition = (multiplier - 1) / 0.5; // Maps 1.0-1.5 to 0-1
        return baseY - normalizedPosition * lowRangePercentage * graphHeight;
    } else {
        // Above 1.5x, use logarithmic scaling for the remaining portion of graph height

        // Steeper log base for more dramatic curve when at higher multipliers
        let logBase = 1.6;
        if (multiplier > 10) {
            logBase = 1.5; // Lower log base = steeper curve
        }
        if (multiplier > 30) {
            logBase = 1.4; // Even steeper for very high multipliers
        }

        // Calculate normalized position using logarithmic scale
        const logMin = Math.log(1.5) / Math.log(logBase);
        const logMax = Math.log(maxDisplayMultiplier) / Math.log(logBase);
        const logVal = Math.log(multiplier) / Math.log(logBase);

        // Normalize to 0-1 range in log space
        const normalizedLogPosition = (logVal - logMin) / (logMax - logMin);

        // Apply the normalized position to the remaining portion of graph height
        // starting from the lowRangePercentage mark
        return (
            baseY - lowRangePercentage * graphHeight - normalizedLogPosition * (1 - lowRangePercentage) * graphHeight
        );
    }
}

// Draw the flight path with the plane leading the curve
export function drawFlightPath(
    gameCanvas: Ref<HTMLCanvasElement>,
    ctx: Ref<CanvasRenderingContext2D>,
    currentMultiplier: Ref<number>,
    shipImage: HTMLImageElement,
    introAnimationActive: Ref<boolean>,
    introProgress: Ref<number>,
    cashouts: Ref<Cashout[]>,
) {
    const width = gameCanvas.value.width;
    const height = gameCanvas.value.height - Y_PADDING;
    const graphHeight = height - X_AXIS_HEIGHT;
    const graphWidth = width - X_PADDING - RIGHT_PADDING;

    // Base Y position (1x)
    const baseY = height - X_AXIS_HEIGHT;

    // Set the rocket position to slightly left from the right edge of the graph area
    const rocketX = width - RIGHT_PADDING - 100; // Added 60px offset from the right edge

    // For the intro animation, we'll just show the plane moving along the baseline
    if (introAnimationActive.value) {
        const introX = X_PADDING + introProgress.value * graphWidth;
        const introY = baseY;
        drawRocket(ctx, currentMultiplier, shipImage, introX, introY);
        return;
    }

    // Calculate the Y position of the rocket based on current multiplier
    const rocketY = mapMultiplierToY(currentMultiplier.value, height, graphHeight) - 20; // Added -60 to move it up

    // Create control points for the curve
    const controlPoints: [number, number][] = [];

    // Start point is always at 1x
    controlPoints.push([X_PADDING, baseY]);

    // For extreme high multipliers, create a more dramatic curve
    if (currentMultiplier.value >= 100) {
        // First 60% of the graph is very flat (multiplier slowly growing)
        controlPoints.push([X_PADDING + graphWidth * 0.3, mapMultiplierToY(1.5, height, graphHeight)]);

        controlPoints.push([X_PADDING + graphWidth * 0.6, mapMultiplierToY(5, height, graphHeight)]);

        // Last 40% is where the dramatic rise happens
        controlPoints.push([X_PADDING + graphWidth * 0.8, mapMultiplierToY(20, height, graphHeight)]);

        controlPoints.push([
            X_PADDING + graphWidth * 0.9,
            mapMultiplierToY(currentMultiplier.value * 0.5, height, graphHeight),
        ]);
    }
    // For high multipliers (20-100x)
    else if (currentMultiplier.value >= 20) {
        controlPoints.push([X_PADDING + graphWidth * 0.4, mapMultiplierToY(2, height, graphHeight)]);

        controlPoints.push([X_PADDING + graphWidth * 0.7, mapMultiplierToY(5, height, graphHeight)]);

        controlPoints.push([X_PADDING + graphWidth * 0.85, mapMultiplierToY(10, height, graphHeight)]);
    }
    // For medium multipliers (5-20x)
    else if (currentMultiplier.value >= 5) {
        controlPoints.push([X_PADDING + graphWidth * 0.3, mapMultiplierToY(1.5, height, graphHeight)]);

        controlPoints.push([X_PADDING + graphWidth * 0.6, mapMultiplierToY(2.5, height, graphHeight)]);

        controlPoints.push([
            X_PADDING + graphWidth * 0.8,
            mapMultiplierToY(currentMultiplier.value * 0.6, height, graphHeight),
        ]);
    }
    // For lower multipliers, use a simpler curve
    else {
        controlPoints.push([
            X_PADDING + graphWidth * 0.5,
            mapMultiplierToY(1 + (currentMultiplier.value - 1) * 0.4, height, graphHeight),
        ]);

        if (currentMultiplier.value > 2) {
            controlPoints.push([
                X_PADDING + graphWidth * 0.75,
                mapMultiplierToY(1 + (currentMultiplier.value - 1) * 0.7, height, graphHeight),
            ]);
        }
    }

    // End point is at the rocket
    controlPoints.push([rocketX, rocketY]);

    // Draw curve glow effect for depth
    // Outer glow
    ctx.value.beginPath();
    ctx.value.moveTo(controlPoints[0][0], controlPoints[0][1]);

    // Draw the curve through the control points
    for (let i = 1; i < controlPoints.length; i++) {
        if (i < controlPoints.length - 1) {
            const xc = (controlPoints[i][0] + controlPoints[i + 1][0]) / 2;
            const yc = (controlPoints[i][1] + controlPoints[i + 1][1]) / 2;
            ctx.value.quadraticCurveTo(controlPoints[i][0], controlPoints[i][1], xc, yc);
        } else {
            ctx.value.lineTo(controlPoints[i][0], controlPoints[i][1]);
        }
    }

    ctx.value.lineWidth = 6;
    ctx.value.strokeStyle = "rgba(77, 255, 77, 0.2)";
    ctx.value.stroke();

    // Inner glow and main curve (same drawing logic)
    drawCurvePath(ctx, controlPoints, 3, "rgba(77, 255, 77, 0.5)");
    drawCurvePath(ctx, controlPoints, 2, "#4dff4d");

    // Draw the rocket at the right edge
    drawRocket(ctx, currentMultiplier, shipImage, rocketX, rocketY);

    // Draw cashout points with better positioning for dramatic curves
    drawCashoutPoints(ctx, cashouts, currentMultiplier, height, graphHeight, graphWidth, baseY);
}

// Helper to draw the curve path (reduces code duplication)
function drawCurvePath(
    ctx: Ref<CanvasRenderingContext2D>,
    points: [number, number][],
    lineWidth: number,
    strokeStyle: string,
) {
    if (!ctx.value) return;

    ctx.value.beginPath();
    ctx.value.moveTo(points[0][0], points[0][1]);

    for (let i = 1; i < points.length; i++) {
        if (i < points.length - 1) {
            const xc = (points[i][0] + points[i + 1][0]) / 2;
            const yc = (points[i][1] + points[i + 1][1]) / 2;
            ctx.value.quadraticCurveTo(points[i][0], points[i][1], xc, yc);
        } else {
            ctx.value.lineTo(points[i][0], points[i][1]);
        }
    }

    ctx.value.lineWidth = lineWidth;
    ctx.value.strokeStyle = strokeStyle;
    ctx.value.stroke();
}

// Draw cashout points with better positioning
function drawCashoutPoints(
    ctx: Ref<CanvasRenderingContext2D>,
    cashouts: Ref<Cashout[]>,
    currentMultiplier: Ref<number>,
    height: number,
    graphHeight: number,
    graphWidth: number,
    baseY: number,
) {
    cashouts.value.forEach((cashout) => {
        // Only show cashouts that are at or below the current multiplier
        if (cashout.multiplier <= currentMultiplier.value) {
            // For higher multipliers, we need special positioning
            let positionRatio;

            if (currentMultiplier.value > 100) {
                // For extreme multipliers, use log scaling for X position
                positionRatio = Math.log(cashout.multiplier) / Math.log(currentMultiplier.value);
                // Adjust to create more natural spacing
                positionRatio = positionRatio * 0.7 + 0.3;
            } else if (currentMultiplier.value > 20) {
                // For high multipliers
                positionRatio = Math.pow(cashout.multiplier / currentMultiplier.value, 0.7);
            } else {
                // For lower multipliers, more linear positioning
                positionRatio = (cashout.multiplier - 1) / (currentMultiplier.value - 1);
            }

            const cx = X_PADDING + positionRatio * graphWidth;
            const cy = mapMultiplierToY(cashout.multiplier, height, graphHeight);

            // Use non-null assertion since we've already checked it above
            const context = ctx.value!;

            // Draw money bag symbol
            context.fillStyle = "#000";
            context.font = "bold 32px Arial";
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText("💰", cx, cy);

            // Draw profit text with better styling
            const profit = Math.floor(cashout.amount * cashout.multiplier - cashout.amount);
            context.fillStyle = "#4dff4d";
            context.font = "bold 18px Arial";
            context.textAlign = "center";
            context.fillText(profit.toString(), cx, cy - 30); // Adjusted from -40 to -45

            // Draw player name
            context.fillStyle = "#FFF";
            context.font = "bold 20px Arial";
            context.fillText(cashout.playerName, cx, cy - 85); // Adjusted from -70 to -85

            // Draw multiplier text
            context.fillStyle = "#FFF";
            context.font = "bold 18px Arial";
            context.fillText(cashout.multiplier.toFixed(2) + "x", cx, cy - 55); // Adjusted from -50 to -55
        }
    });
}

// Draw a spaceship with Mario Party styling
function drawRocket(
    ctx: Ref<CanvasRenderingContext2D>,
    currentMultiplier: Ref<number>,
    shipImage: HTMLImageElement,
    x: number,
    y: number,
) {
    // Apply a more playful vertical bobbing effect
    y += Math.sin(Date.now() / 150) * 3;

    // Generate particles for the trail
    if (engineParticles.length < 40) {
        // Add new particles at the engine position with more variety
        engineParticles.push({
            x: x - 30, // Behind spaceship
            y: y + (Math.random() * 14 - 7), // Wider spread
            size: 5 + Math.random() * 10, // Varied particle sizes
            opacity: 0.6 + Math.random() * 0.4,
            speed: 2 + Math.random() * 5, // Faster particles
        });
    }

    // Draw particles
    if (engineParticles.length > 0) {
        for (let i = engineParticles.length - 1; i >= 0; i--) {
            const p = engineParticles[i];
            // Update particle
            p.x -= p.speed;
            p.size *= 0.96;
            p.opacity *= 0.96;

            // Draw Mario-styled particles
            if (ctx.value) {
                const particleType = i % 3; // 0 = star, 1 = coin, 2 = sparkle

                ctx.value.save();
                ctx.value.translate(p.x, p.y);

                if (particleType === 0) {
                    // Star particle
                    const angleOffset = (Date.now() / 80) % (Math.PI * 2);
                    ctx.value.rotate(angleOffset);

                    const starPoints = 5;
                    const outerRadius = p.size * 1.8;
                    const innerRadius = p.size * 0.7;

                    ctx.value.beginPath();
                    for (let j = 0; j < starPoints * 2; j++) {
                        const radius = j % 2 === 0 ? outerRadius : innerRadius;
                        const angle = (j * Math.PI) / starPoints;

                        if (j === 0) {
                            ctx.value.moveTo(radius, 0);
                        } else {
                            ctx.value.lineTo(radius * Math.cos(angle), radius * Math.sin(angle));
                        }
                    }
                    ctx.value.closePath();

                    // Star gradient - Mario gold star colors
                    const gradient = ctx.value.createRadialGradient(0, 0, innerRadius, 0, 0, outerRadius);
                    gradient.addColorStop(0, `rgba(255, 255, 160, ${p.opacity})`);
                    gradient.addColorStop(0.6, `rgba(255, 220, 0, ${p.opacity})`);
                    gradient.addColorStop(1, `rgba(255, 150, 0, ${p.opacity * 0.7})`);

                    ctx.value.fillStyle = gradient;
                    ctx.value.fill();
                } else if (particleType === 1) {
                    // Coin particle
                    const coinScaleX = Math.abs(Math.sin(Date.now() / 200 + i)) * 0.5 + 0.5;
                    ctx.value.scale(coinScaleX, 1);

                    ctx.value.beginPath();
                    ctx.value.arc(0, 0, p.size * 1.5, 0, Math.PI * 2);

                    const coinGradient = ctx.value.createRadialGradient(0, 0, 0, 0, 0, p.size * 1.5);
                    coinGradient.addColorStop(0, `rgba(255, 240, 130, ${p.opacity})`);
                    coinGradient.addColorStop(0.8, `rgba(255, 200, 0, ${p.opacity})`);
                    coinGradient.addColorStop(1, `rgba(200, 150, 0, ${p.opacity * 0.5})`);

                    ctx.value.fillStyle = coinGradient;
                    ctx.value.fill();
                } else {
                    // Sparkle particle
                    const sparkTime = Date.now() / 100;
                    const sparkSize = p.size * 1.3;

                    ctx.value.rotate(sparkTime % (Math.PI * 2));

                    ctx.value.beginPath();
                    ctx.value.moveTo(0, -sparkSize);
                    ctx.value.lineTo(0, sparkSize);
                    ctx.value.moveTo(-sparkSize, 0);
                    ctx.value.lineTo(sparkSize, 0);

                    ctx.value.rotate(Math.PI / 4);
                    ctx.value.moveTo(0, -sparkSize * 0.7);
                    ctx.value.lineTo(0, sparkSize * 0.7);
                    ctx.value.moveTo(-sparkSize * 0.7, 0);
                    ctx.value.lineTo(sparkSize * 0.7, 0);

                    ctx.value.strokeStyle = `rgba(255, 255, 255, ${p.opacity})`;
                    ctx.value.lineWidth = 2;
                    ctx.value.stroke();
                }

                ctx.value.restore();
            }

            // Remove faded particles
            if (p.opacity < 0.1 || p.size < 0.5) {
                engineParticles.splice(i, 1);
            }
        }
    }

    // Draw the ship image
    ctx.value.save();
    ctx.value.translate(x, y);

    // Start at 0.8 radians and gradually decrease to 0 based on multiplier
    const baseRotation = 0.8;
    const multiplierFactor = Math.min(1, (currentMultiplier.value - 1) / 4); // Will reach 0 at 5x
    const currentRotation = baseRotation * (1 - multiplierFactor);

    // Add playful tilt animation
    const tiltAngle = Math.sin(Date.now() / 800) * 0.08;
    ctx.value.rotate(currentRotation + tiltAngle);

    // Draw the ship image with larger dimensions
    const shipWidth = 240;
    const shipHeight = 240;
    ctx.value.drawImage(shipImage, -shipWidth / 2, -shipHeight / 2, shipWidth, shipHeight);

    ctx.value.restore();
}

// Define missing types and variables at the top of the script section
interface Particle {
    x: number;
    y: number;
    size: number;
    color: string;
    vx: number;
    vy: number;
    life: number;
}

interface ExplosionParticle {
    x: number;
    y: number;
    targetX: number;
    targetY: number;
    size: number;
    color: string;
    progress: number;
    speed: number;
    rotationSpeed: number;
    rotation: number;
    shape: "star" | "circle" | "square";
}

// Variables for particle effects
let engineParticles: StarParticle[] = []; // For rocket trail in drawRocket

// Define missing types and variables at the top of the script section
// Original particle type for background stars
interface StarParticle {
    x: number;
    y: number;
    size: number;
    opacity: number;
    speed: number;
}

// Ship engine and explosion particle type
interface Particle {
    x: number;
    y: number;
    size: number;
    color: string;
    vx: number;
    vy: number;
    life: number;
}

// Add background effects
export function addBackgroundEffects(
    ctx: Ref<CanvasRenderingContext2D>,
    gameCanvas: Ref<HTMLCanvasElement>,
    gameEnded: Ref<boolean>,
) {
    if (!ctx.value || !gameCanvas.value) return;

    const width = gameCanvas.value.width;
    const height = gameCanvas.value.height - Y_PADDING;

    // Create background gradient
    const spaceGradient = ctx.value.createLinearGradient(0, 0, 0, height);

    if (gameEnded.value) {
        spaceGradient.addColorStop(0, "#CC0000");
        spaceGradient.addColorStop(0.5, "#AA0000");
        spaceGradient.addColorStop(1, "#660000");
    } else {
        spaceGradient.addColorStop(0, "#5438BB");
        spaceGradient.addColorStop(0.7, "#3850BB");
        spaceGradient.addColorStop(1, "#1A2D99");
    }

    ctx.value.fillStyle = spaceGradient;
    ctx.value.fillRect(0, 0, width, height);

    // Add grid
    ctx.value.strokeStyle = "rgba(255, 255, 255, 0.1)";
    ctx.value.lineWidth = 1;

    const gridSize = 40;
    const gridWidth = Math.ceil(width / gridSize);
    const gridHeight = Math.ceil(height / gridSize);

    for (let i = 0; i <= gridWidth; i++) {
        ctx.value.beginPath();
        ctx.value.moveTo(i * gridSize, 0);
        ctx.value.lineTo(i * gridSize, height);
        ctx.value.stroke();
    }

    for (let i = 0; i <= gridHeight; i++) {
        ctx.value.beginPath();
        ctx.value.moveTo(0, i * gridSize);
        ctx.value.lineTo(width, i * gridSize);
        ctx.value.stroke();
    }
}
