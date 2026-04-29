const MIN_SIZES = [16, 32, 64, 128, 256, 512];
const LINE_WEIGHTS = [0.5, 1, 1.5, 2, 2.5, 3];
const DEFAULT_THRESHOLD = 80;
const DEFAULT_LINE_WEIGHT_INDEX = 1;
const MIN_BLOB_AREA = 50;
const DEFAULT_ASPECT_RATIO = 4 / 3;

let minBoxSize = MIN_SIZES[0];
let lineWeight = LINE_WEIGHTS[DEFAULT_LINE_WEIGHT_INDEX];

let video = null;
let threshSlider;
let threshLabel;
let minSizeSlider;
let minSizeLabel;
let lineWeightSlider;
let lineWeightLabel;
let fileInput;
let videoLoaded = false;
let currentAspectRatio = DEFAULT_ASPECT_RATIO;
let binaryMaskBuffer = new Uint8Array(0);
let visitedBuffer = new Uint8Array(0);

function setup() {
    const canvas = createCanvas(640, 480);
    canvas.parent('canvasContainer');
    pixelDensity(1);

    buildControls();
    updateCanvasSize();
}

function buildControls() {
    const controlsPanel = select('#controlsPanel');

    createImportControl(controlsPanel);

    ({
        slider: threshSlider,
        label: threshLabel
    } = createSliderControl({
        parent: controlsPanel,
        labelText: 'Threshold',
        min: 0,
        max: 255,
        value: DEFAULT_THRESHOLD,
        step: 1,
        onInput: (slider, label) => {
            label.html(formatControlLabel('Threshold', slider.value()));
        }
    }));

    ({
        slider: minSizeSlider,
        label: minSizeLabel
    } = createSliderControl({
        parent: controlsPanel,
        labelText: 'Min size',
        min: 0,
        max: MIN_SIZES.length - 1,
        value: 0,
        step: 1,
        displayValue: minBoxSize,
        onInput: (slider, label) => {
            minBoxSize = MIN_SIZES[slider.value()];
            label.html(formatControlLabel('Min size', minBoxSize));
        }
    }));

    ({
        slider: lineWeightSlider,
        label: lineWeightLabel
    } = createSliderControl({
        parent: controlsPanel,
        labelText: 'Line weight',
        min: 0,
        max: LINE_WEIGHTS.length - 1,
        value: DEFAULT_LINE_WEIGHT_INDEX,
        step: 1,
        displayValue: lineWeight,
        onInput: (slider, label) => {
            lineWeight = LINE_WEIGHTS[slider.value()];
            label.html(formatControlLabel('Line weight', lineWeight));
        }
    }));
}

function createImportControl(parent) {
    const importGroup = createDiv('');
    importGroup.addClass('control-group');
    importGroup.parent(parent);

    fileInput = createFileInput(handleFile);
    fileInput.parent(importGroup);
    fileInput.style('display', 'none');

    const importButton = createButton('Import video');
    importButton.addClass('import-button');
    importButton.parent(importGroup);
    importButton.mousePressed(() => fileInput.elt.click());

    createDivider(parent);
}

function createDivider(parent) {
    const divider = createElement('hr');
    divider.addClass('sidebar-divider');
    divider.parent(parent);
    return divider;
}

function createSliderControl({
    parent,
    labelText,
    min,
    max,
    value,
    step,
    displayValue = value,
    onInput
}) {
    const group = createDiv('');
    group.addClass('control-group');
    group.parent(parent);

    const label = createDiv(formatControlLabel(labelText, displayValue));
    label.addClass('control-label');
    label.parent(group);

    const slider = createSlider(min, max, value, step);
    slider.addClass('control-slider');
    slider.parent(group);
    slider.input(() => onInput(slider, label));

    return { slider, label };
}

function formatControlLabel(label, value) {
    return `${label}: <strong>${value}</strong>`;
}

function updateCanvasSize() {
    const canvasContainer = select('#canvasContainer');
    if (!canvasContainer) return;

    const bounds = canvasContainer.elt.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return;

    let targetWidth = bounds.width;
    let targetHeight = targetWidth / currentAspectRatio;

    if (targetHeight > bounds.height) {
        targetHeight = bounds.height;
        targetWidth = targetHeight * currentAspectRatio;
    }

    resizeCanvas(
        Math.max(1, Math.floor(targetWidth)),
        Math.max(1, Math.floor(targetHeight))
    );

    if (video) {
        video.size(width, height);
    }
}

function windowResized() {
    updateCanvasSize();
}

function handleFile(file) {
    if (!file || file.type !== 'video') return;

    if (video) {
        video.remove();
    }

    videoLoaded = false;

    const isObjectURL = !file.data;
    const source = file.data || URL.createObjectURL(file.file);
    video = createVideo([source], () => {
        if (isObjectURL) {
            URL.revokeObjectURL(source);
        }

        const nativeWidth = video.elt.videoWidth || width;
        const nativeHeight = video.elt.videoHeight || height;

        currentAspectRatio = nativeHeight > 0
            ? nativeWidth / nativeHeight
            : DEFAULT_ASPECT_RATIO;

        updateCanvasSize();
        video.size(width, height);
        video.volume(0);
        video.elt.muted = true;
        video.loop();
        video.hide();
        videoLoaded = true;
    });
}

function draw() {
    background(0);

    if (!videoLoaded) {
        drawEmptyState();
        return;
    }

    image(video, 0, 0, width, height);
    video.loadPixels();

    if (!video.pixels || video.pixels.length === 0) return;

    const visibleBlobs = getVisibleBlobs(video, threshSlider.value());
    drawBlobConnections(visibleBlobs);
    drawBlobBoxes(visibleBlobs);
}

function drawEmptyState() {
    fill(255);
    noStroke();
    textSize(18);
    textAlign(CENTER, CENTER);
    text('Import a video', width / 2, height / 2);
}

function getVisibleBlobs(sourceVideo, threshold) {
    const binaryMask = buildBinaryMask(sourceVideo, threshold);
    const blobs = findBlobs(binaryMask, sourceVideo.width, sourceVideo.height);

    return blobs
        .filter((blob) => isVisibleBlob(blob, sourceVideo.width, sourceVideo.height))
        .sort((a, b) => a.area - b.area || a.centerX - b.centerX || a.centerY - b.centerY);
}

function buildBinaryMask(sourceVideo, threshold) {
    const { width: videoWidth, height: videoHeight, pixels } = sourceVideo;
    const requiredSize = videoWidth * videoHeight;

    if (binaryMaskBuffer.length !== requiredSize) {
        binaryMaskBuffer = new Uint8Array(requiredSize);
    }

    for (let y = 0; y < videoHeight; y++) {
        for (let x = 0; x < videoWidth; x++) {
            const pixelIndex = x + y * videoWidth;
            const colorIndex = pixelIndex * 4;
            const r = pixels[colorIndex];
            const g = pixels[colorIndex + 1];
            const b = pixels[colorIndex + 2];
            const luminance = (r + g + b) / 3;

            binaryMaskBuffer[pixelIndex] = luminance > threshold ? 1 : 0;
        }
    }

    return binaryMaskBuffer;
}

function findBlobs(binaryMask, width, height) {
    const requiredSize = width * height;

    if (visitedBuffer.length !== requiredSize) {
        visitedBuffer = new Uint8Array(requiredSize);
    } else {
        visitedBuffer.fill(0);
    }

    const blobs = [];

    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const index = x + y * width;

            if (binaryMask[index] === 1 && visitedBuffer[index] === 0) {
                const blob = floodFillBlob(x, y, binaryMask, visitedBuffer, width, height);

                if (blob.area > MIN_BLOB_AREA) {
                    blobs.push(blob);
                }
            }
        }
    }

    return blobs;
}

function floodFillBlob(startX, startY, binaryMask, visited, width, height) {
    const queue = [[startX, startY]];
    let minX = startX;
    let minY = startY;
    let maxX = startX;
    let maxY = startY;
    let sumX = 0;
    let sumY = 0;
    let count = 0;

    while (queue.length > 0) {
        const [x, y] = queue.pop();

        if (x < 0 || x >= width || y < 0 || y >= height) {
            continue;
        }

        const index = x + y * width;
        if (visited[index]) continue;

        visited[index] = true;
        if (binaryMask[index] !== 1) continue;

        sumX += x;
        sumY += y;
        count++;
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);

        queue.push([x - 1, y]);
        queue.push([x + 1, y]);
        queue.push([x, y - 1]);
        queue.push([x, y + 1]);
    }

    return {
        minX,
        minY,
        maxX,
        maxY,
        centerX: sumX / count,
        centerY: sumY / count,
        area: count
    };
}

function isVisibleBlob(blob, videoWidth, videoHeight) {
    const boxWidth = blob.maxX - blob.minX + 1;
    const boxHeight = blob.maxY - blob.minY + 1;

    return (
        boxWidth >= minBoxSize &&
        boxHeight >= minBoxSize &&
        boxWidth <= videoWidth / 2 &&
        boxHeight <= videoHeight / 2
    );
}

function drawBlobConnections(blobs) {
    stroke('red');
    strokeWeight(lineWeight);

    for (let i = 0; i < blobs.length - 1; i++) {
        const currentBlob = blobs[i];
        const nextBlob = blobs[i + 1];

        line(
            currentBlob.centerX,
            currentBlob.centerY,
            nextBlob.centerX,
            nextBlob.centerY
        );
    }
}

function drawBlobBoxes(blobs) {
    noFill();
    stroke('red');
    strokeWeight(lineWeight);
    textSize(12);

    for (const blob of blobs) {
        const boxWidth = blob.maxX - blob.minX + 1;
        const boxHeight = blob.maxY - blob.minY + 1;
        const ratio = boxWidth > 0 ? (boxHeight / boxWidth).toFixed(4) : '0.0000';

        rect(blob.minX, blob.minY, boxWidth, boxHeight);

        push();
        fill('red');
        noStroke();
        textAlign(LEFT, BOTTOM);
        text(ratio, blob.minX, blob.minY);
        pop();
    }
}
