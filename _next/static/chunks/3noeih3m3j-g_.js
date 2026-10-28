(globalThis.TURBOPACK||(globalThis.TURBOPACK=[])).push(["object"==typeof document?document.currentScript:void 0,44498,e=>{"use strict";var o,i,r,n,t=e.i(43476),l=e.i(71645),s=e.i(75056),a=e.i(25234);function c(){return(c=Object.assign.bind()).apply(null,arguments)}var f=e.i(90072),d=e.i(1950),d=d,m=f;let u=parseInt(f.REVISION.replace(/\D+/g,"")),g=(o={cellSize:.5,sectionSize:1,fadeDistance:100,fadeStrength:1,fadeFrom:1,cellThickness:.5,sectionThickness:1,cellColor:new f.Color,sectionColor:new f.Color,infiniteGrid:!1,followCamera:!1,worldCamProjPosition:new f.Vector3,worldPlanePosition:new f.Vector3},i=`
    varying vec3 localPosition;
    varying vec4 worldPosition;

    uniform vec3 worldCamProjPosition;
    uniform vec3 worldPlanePosition;
    uniform float fadeDistance;
    uniform bool infiniteGrid;
    uniform bool followCamera;

    void main() {
      localPosition = position.xzy;
      if (infiniteGrid) localPosition *= 1.0 + fadeDistance;
      
      worldPosition = modelMatrix * vec4(localPosition, 1.0);
      if (followCamera) {
        worldPosition.xyz += (worldCamProjPosition - worldPlanePosition);
        localPosition = (inverse(modelMatrix) * worldPosition).xyz;
      }

      gl_Position = projectionMatrix * viewMatrix * worldPosition;
    }
  `,r=`
    varying vec3 localPosition;
    varying vec4 worldPosition;

    uniform vec3 worldCamProjPosition;
    uniform float cellSize;
    uniform float sectionSize;
    uniform vec3 cellColor;
    uniform vec3 sectionColor;
    uniform float fadeDistance;
    uniform float fadeStrength;
    uniform float fadeFrom;
    uniform float cellThickness;
    uniform float sectionThickness;

    float getGrid(float size, float thickness) {
      vec2 r = localPosition.xz / size;
      vec2 grid = abs(fract(r - 0.5) - 0.5) / fwidth(r);
      float line = min(grid.x, grid.y) + 1.0 - thickness;
      return 1.0 - min(line, 1.0);
    }

    void main() {
      float g1 = getGrid(cellSize, cellThickness);
      float g2 = getGrid(sectionSize, sectionThickness);

      vec3 from = worldCamProjPosition*vec3(fadeFrom);
      float dist = distance(from, worldPosition.xyz);
      float d = 1.0 - min(dist / fadeDistance, 1.0);
      vec3 color = mix(cellColor, sectionColor, min(1.0, sectionThickness * g2));

      gl_FragColor = vec4(color, (g1 + g2) * pow(d, fadeStrength));
      gl_FragColor.a = mix(0.75 * gl_FragColor.a, gl_FragColor.a, g2);
      if (gl_FragColor.a <= 0.0) discard;

      #include <tonemapping_fragment>
      #include <${u>=154?"colorspace_fragment":"encodings_fragment"}>
    }
  `,(n=class extends m.ShaderMaterial{constructor(e){for(const n in super({vertexShader:i,fragmentShader:r,...e}),o)this.uniforms[n]=new m.Uniform(o[n]),Object.defineProperty(this,n,{get(){return this.uniforms[n].value},set(e){this.uniforms[n].value=e}});this.uniforms=m.UniformsUtils.clone(this.uniforms)}}).key=m.MathUtils.generateUUID(),n),p=l.forwardRef(({args:e,cellColor:o="#000000",sectionColor:i="#2080ff",cellSize:r=.5,sectionSize:n=1,followCamera:t=!1,infiniteGrid:s=!1,fadeDistance:m=100,fadeStrength:u=1,fadeFrom:p=1,cellThickness:x=.5,sectionThickness:h=1,side:P=f.BackSide,...y},v)=>{(0,d.e)({GridMaterial:g});let w=l.useRef(null);l.useImperativeHandle(v,()=>w.current,[]);let C=new f.Plane,j=new f.Vector3(0,1,0),S=new f.Vector3(0,0,0);return(0,a.useFrame)(e=>{C.setFromNormalAndCoplanarPoint(j,S).applyMatrix4(w.current.matrixWorld);let o=w.current.material,i=o.uniforms.worldCamProjPosition,r=o.uniforms.worldPlanePosition;C.projectPoint(e.camera.position,i.value),r.value.set(0,0,0).applyMatrix4(w.current.matrixWorld)}),l.createElement("mesh",c({ref:w,frustumCulled:!1},y),l.createElement("gridMaterial",c({transparent:!0,"extensions-derivatives":!0,side:P},{cellSize:r,sectionSize:n,cellColor:o,sectionColor:i,cellThickness:x,sectionThickness:h},{fadeDistance:m,fadeStrength:u,fadeFrom:p,infiniteGrid:s,followCamera:t})),l.createElement("planeGeometry",{args:e}))});function x({pointer:e,children:o}){let i=(0,l.useRef)(null),r=(0,l.useRef)({x:0,y:0});return(0,a.useFrame)((o,n)=>{let t=i.current;if(!t)return;let l=e.current??{x:0,y:0},s=1-Math.exp(-4*n);r.current.x+=(l.x-r.current.x)*s,r.current.y+=(l.y-r.current.y)*s;let a=o.clock.elapsedTime;t.rotation.y=.1*Math.sin(.08*a)+.14*r.current.x,t.rotation.x=.05*r.current.y}),(0,t.jsx)("group",{ref:i,children:o})}function h({colors:e}){return(0,t.jsxs)("group",{children:[(0,t.jsx)(y,{geometry:"box",args:[8.4,4.6,.12],position:[0,2.3,-3.6],color:e.line}),(0,t.jsx)(y,{geometry:"box",args:[1.5,1.5,1.5],position:[-2.5,.75,-.8],color:e.lineStrong}),(0,t.jsx)(y,{geometry:"box",args:[1,1,1],position:[-1.35,.5,.6],color:e.line}),(0,t.jsx)(y,{geometry:"cylinder",args:[.55,.55,1.4,12],position:[2.6,.7,-1.2],color:e.lineStrong}),(0,t.jsx)(y,{geometry:"sphere",args:[.62,16,12],position:[1.5,.62,.9],color:e.line}),(0,t.jsx)(y,{geometry:"capsule",args:[.34,1.05,4,10],position:[.1,.86,-.2],color:e.accent}),(0,t.jsx)(v,{colors:e})]})}let P={box:f.BoxGeometry,cylinder:f.CylinderGeometry,sphere:f.SphereGeometry,capsule:f.CapsuleGeometry};function y({geometry:e,args:o,position:i,color:r,rotation:n}){let s=(0,l.useMemo)(()=>{let i=new P[e](...o),r=new f.EdgesGeometry(i,18);return i.dispose(),r},[e,...o]);return(0,t.jsx)("lineSegments",{geometry:s,position:i,rotation:n,children:(0,t.jsx)("lineBasicMaterial",{color:r,transparent:!0,opacity:.9})})}function v({colors:e}){return(0,t.jsxs)("group",{position:[3.4,1.5,3],rotation:[0,Math.PI/4.6,0],children:[(0,t.jsx)(y,{geometry:"box",args:[.55,.42,.8],position:[0,0,0],color:e.accent}),(0,t.jsx)(y,{geometry:"cylinder",args:[.34,.06,.55,4],position:[0,0,-.68],rotation:[-Math.PI/2,Math.PI/4,0],color:e.accent})]})}e.s(["default",0,function({colors:e,pointer:o}){return(0,t.jsx)(s.Canvas,{"aria-hidden":!0,tabIndex:-1,dpr:[1,2],gl:{antialias:!0,alpha:!0,powerPreference:"high-performance"},camera:{fov:34,position:[0,2.1,9.2],near:.1,far:60},style:{pointerEvents:"none"},children:(0,t.jsxs)(x,{pointer:o,children:[(0,t.jsx)(p,{position:[0,0,0],args:[30,30],cellSize:.6,cellThickness:.6,cellColor:e.line,sectionSize:3,sectionThickness:1,sectionColor:e.lineStrong,fadeDistance:26,fadeStrength:2.4,infiniteGrid:!0}),(0,t.jsx)(h,{colors:e})]})})}],44498)},47055,e=>{e.n(e.i(44498))}]);