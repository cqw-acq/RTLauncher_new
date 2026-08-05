var _gl = typeof glMatrix !== 'undefined' ? glMatrix : (window.glMatrix || {});
var mat4 = _gl.mat4;
var vec3 = _gl.vec3;

var webglContext;
var deepslateRenderer;
var cameraPitch;
var cameraYaw;
var cameraPos;

function setStructure(structure, reset_view) {
  if (reset_view === undefined) reset_view = false;
  deepslateRenderer = new deepslate.StructureRenderer(webglContext, structure, deepslateResources, {chunkSize: 8});
  if (reset_view) {
    cameraPitch = 0.8;
    cameraYaw = 0.5;
    var size = structure.getSize();
    vec3.set(cameraPos, -size[0] / 2, -size[1] / 2, -size[2] / 2);
  }
  requestAnimationFrame(render);
}

function render() {
  cameraYaw = cameraYaw % (Math.PI * 2);
  cameraPitch = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, cameraPitch));
  var view = mat4.create();
  mat4.rotateX(view, view, cameraPitch);
  mat4.rotateY(view, view, cameraYaw);
  mat4.translate(view, view, cameraPos);
  deepslateRenderer.drawStructure(view);
  deepslateRenderer.drawGrid(view);
}

var _keyInterval = null;
var _keyDownHandler = null;
var _keyUpHandler = null;
var _blurHandler = null;

function createRenderCanvas(canvas) {
  webglContext = canvas.getContext('webgl');
  if (!webglContext) {
    console.error("[schematic-viewer] WebGL not available");
    return;
  }

  cameraPitch = 0.8;
  cameraYaw = 0.5;
  cameraPos = vec3.create();

  function move3d(direction, relativeVertical, sensitivity) {
    if (relativeVertical === undefined) relativeVertical = true;
    if (sensitivity === undefined) sensitivity = 1;
    var offset = vec3.create();
    vec3.set(offset, direction[0] * sensitivity, direction[1] * sensitivity, direction[2] * sensitivity);
    if (relativeVertical) {
      vec3.rotateX(offset, offset, [0, 0, 0], -cameraPitch * sensitivity);
    }
    vec3.rotateY(offset, offset, [0, 0, 0], -cameraYaw * sensitivity);
    vec3.add(cameraPos, cameraPos, offset);
  }

  function pan(direction, sensitivity) {
    if (sensitivity === undefined) sensitivity = 1;
    cameraYaw += direction[0] / 200 * sensitivity;
    cameraPitch += direction[1] / 200 * sensitivity;
  }

  function move(offset, sensitivity) {
    if (sensitivity === undefined) sensitivity = 1;
    var xOffset = offset[0] / 500 * sensitivity;
    var yOffset = offset[1] / 500 * sensitivity;
    var offset_vector = vec3.create();
    vec3.set(offset_vector, xOffset, -yOffset, 0);
    vec3.rotateX(offset_vector, offset_vector, [0, 0, 0], -cameraPitch);
    vec3.rotateY(offset_vector, offset_vector, [0, 0, 0], -cameraYaw);
    vec3.add(cameraPos, cameraPos, offset_vector);
  }

  function runMovementFunction(setting, args, controls, invertSetting, sensitivitySetting) {
    var value = 'pan';
    try { value = localStorage.getItem(setting) || 'pan'; } catch(e) {}
    var sensitivity = 1;
    if (sensitivitySetting) {
      try { sensitivity *= parseFloat(localStorage.getItem(sensitivitySetting) || '1'); } catch(e) {}
    }
    if (invertSetting) {
      try {
        var invert = localStorage.getItem(invertSetting) === 'true';
        if (invert) sensitivity *= -1;
      } catch(e) {}
    }
    if (controls[value]) {
      controls[value](args, sensitivity);
    }
  }

  var middleClickPos = null;
  var leftPos = null;

  canvas.addEventListener('mousedown', function(evt) {
    if (evt.button === 0) {
      evt.preventDefault();
      leftPos = [evt.clientX, evt.clientY];
    } else if (evt.button === 1) {
      evt.preventDefault();
      middleClickPos = [evt.clientX, evt.clientY];
    }
  });

  canvas.addEventListener('mousemove', function(evt) {
    if (middleClickPos) {
      var args = [evt.clientX - middleClickPos[0], evt.clientY - middleClickPos[1]];
      runMovementFunction('middle-click-drag', args, {move: move, pan: pan}, 'middle-click-drag-invert', 'middle-click-drag-sensitivity');
      middleClickPos = [evt.clientX, evt.clientY];
      requestAnimationFrame(render);
    } else if (leftPos) {
      var args = [evt.clientX - leftPos[0], evt.clientY - leftPos[1]];
      runMovementFunction('click-drag', args, {move: move, pan: pan}, 'click-drag-invert', 'click-drag-sensitivity');
      leftPos = [evt.clientX, evt.clientY];
      requestAnimationFrame(render);
    }
  });

  canvas.addEventListener('mouseup', function(evt) {
    if (evt.button === 0) {
      leftPos = null;
    } else if (evt.button === 1) {
      middleClickPos = null;
      evt.preventDefault();
    }
  });

  canvas.addEventListener('mouseout', function() {
    leftPos = null;
    middleClickPos = null;
  });

  canvas.addEventListener('wheel', function(evt) {
    evt.preventDefault();
    move3d([0, 0, -evt.deltaY / 200]);
    requestAnimationFrame(render);
  });

  canvas.addEventListener('contextmenu', function(evt) {
    evt.preventDefault();
  });

  var moveDist = 0.2;
  var keyMoves = {
    KeyW: [0, 0, moveDist],
    KeyS: [0, 0, -moveDist],
    KeyA: [moveDist, 0, 0],
    KeyD: [-moveDist, 0, 0],
    ArrowUp: [0, 0, moveDist],
    ArrowDown: [0, 0, -moveDist],
    ArrowLeft: [moveDist, 0, 0],
    ArrowRight: [-moveDist, 0, 0],
    ShiftLeft: [0, moveDist, 0],
    Space: [0, -moveDist, 0]
  };

  var pressedKeys = new Set();

  _keyDownHandler = function(evt) {
    if (evt.code in keyMoves) {
      evt.preventDefault();
      pressedKeys.add(evt.code);
    }
  };

  _keyUpHandler = function(evt) {
    pressedKeys.delete(evt.code);
  };

  _blurHandler = function() {
    pressedKeys.clear();
  };

  document.addEventListener('keydown', _keyDownHandler);
  document.addEventListener('keyup', _keyUpHandler);
  window.addEventListener('blur', _blurHandler);

  if (_keyInterval) clearInterval(_keyInterval);
  _keyInterval = setInterval(function() {
    if (pressedKeys.size === 0) return;
    var direction = vec3.create();
    pressedKeys.forEach(function(key) {
      if (keyMoves[key]) {
        vec3.add(direction, direction, keyMoves[key]);
      }
    });
    move3d(direction, false);
    requestAnimationFrame(render);
  }, 1000 / 60);

  var pinchSpeed = 0.015;
  var dragSpeed = 0.01;
  var prevAvgX = null;
  var prevAvgY = null;
  var prevDist = null;

  function touchHandler(evt) {
    evt.preventDefault();
    if (evt.touches.length === 1) {
      if (evt.touches && middleClickPos) {
        var dx = evt.touches[0].pageX - middleClickPos[0];
        var dy = evt.touches[0].pageY - middleClickPos[1];
        pan([dx, dy]);
        requestAnimationFrame(render);
      }
      middleClickPos = [evt.touches[0].pageX, evt.touches[0].pageY];
    } else if (evt.touches.length === 2) {
      var dx = evt.touches[0].pageX - evt.touches[1].pageX;
      var dy = evt.touches[0].pageY - evt.touches[1].pageY;
      var dist = Math.sqrt(dx * dx + dy * dy);
      if (prevDist === null) prevDist = dist;
      var avgX = (evt.touches[0].pageX + evt.touches[1].pageX) / 2;
      var avgY = (evt.touches[0].pageY + evt.touches[1].pageY) / 2;
      if (prevAvgX === null) prevAvgX = avgX;
      if (prevAvgY === null) prevAvgY = avgY;
      var distX = (avgX - prevAvgX) * dragSpeed;
      var distY = (prevAvgY - avgY) * dragSpeed;
      move3d([distX, distY, (dist - prevDist) * pinchSpeed]);
      requestAnimationFrame(render);
      prevDist = dist;
      prevAvgX = avgX;
      prevAvgY = avgY;
    }
  }

  canvas.addEventListener('touchstart', touchHandler, { passive: false });
  canvas.addEventListener('touchmove', touchHandler, { passive: false });
  canvas.addEventListener('touchend', function() {
    middleClickPos = null;
    prevDist = null;
    prevAvgX = null;
    prevAvgY = null;
  });
}

function destroyRenderCanvas() {
  if (_keyInterval) {
    clearInterval(_keyInterval);
    _keyInterval = null;
  }
  if (_keyDownHandler) {
    document.removeEventListener('keydown', _keyDownHandler);
    _keyDownHandler = null;
  }
  if (_keyUpHandler) {
    document.removeEventListener('keyup', _keyUpHandler);
    _keyUpHandler = null;
  }
  if (_blurHandler) {
    window.removeEventListener('blur', _blurHandler);
    _blurHandler = null;
  }
  deepslateRenderer = null;
  webglContext = null;
}