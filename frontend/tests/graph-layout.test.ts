import {test} from 'node:test';
import assert from 'node:assert/strict';
import {arrangeGraph,boundsOf,connectedComponents,fitCamera,neighbourhood,routeLinks,zoomCamera,type GraphLink} from '../src/graph-layout.ts';

const edge=(source:string,target:string,id=`${source}:${target}`):GraphLink=>({id,source,target,record:source,label:'0'});

test('dense 250-node layout grows beyond the old rectangle without overlapping nodes',()=>{
  const ids=Array.from({length:250},(_,i)=>String(i));
  const links=ids.slice(1).map(id=>edge('0',id));
  const copy=JSON.stringify(links),layout=arrangeGraph(ids,links,false);
  assert.equal(layout.nodes.length,250);
  assert.equal(JSON.stringify(links),copy,'D3 must only mutate copied links');
  const bounds=boundsOf(layout.nodes,0);
  assert.ok(bounds.maxX-bounds.minX>810||bounds.maxY-bounds.minY>440);
  for(let i=0;i<layout.nodes.length;i++)for(let j=0;j<i;j++){
    const a=layout.nodes[i],b=layout.nodes[j];
    assert.ok(Math.hypot(a.x-b.x,a.y-b.y)>70,`overlap: ${a.id} / ${b.id}`);
  }
  assert.deepEqual(arrangeGraph(ids,links,false),layout,'fresh layouts are deterministic');
  const unrelatedPositions=new Map(ids.map(id=>[id,{x:Number(id)*100,y:-Number(id)*700}]));
  assert.deepEqual(arrangeGraph(ids,links,false,unrelatedPositions),layout,'filter history cannot distort an unpinned layout');
});

test('disconnected rule representations are packed separately without losing identities',()=>{
  const ids=Array.from({length:120},(_,i)=>String(i));
  const links=ids.filter((_,i)=>i%30!==0).map(id=>edge(String(Math.floor(Number(id)/30)*30),id));
  const layout=arrangeGraph(ids,links,false);
  assert.equal(layout.components.length,4);
  assert.deepEqual(new Set(layout.nodes.map(n=>n.id)),new Set(ids));
  for(let i=0;i<layout.components.length;i++)for(let j=0;j<i;j++){
    const a=layout.components[i].bounds,b=layout.components[j].bounds;
    assert.ok(a.maxX<b.minX||b.maxX<a.minX||a.maxY<b.minY||b.maxY<a.minY);
  }
  assert.deepEqual(connectedComponents(['a','b','c'],[edge('b','a'),edge('b','b')]),[['a','b'],['c']]);
});

test('fit frames actual bounds in both wide and narrow viewports',()=>{
  const bounds={minX:-2200,minY:-800,maxX:4300,maxY:2700};
  for(const [width,height]of [[1100,600],[340,480]]){
    const camera=fitCamera(bounds,width,height);
    for(const [x,y]of [[bounds.minX,bounds.minY],[bounds.maxX,bounds.maxY]]){
      const screenX=width/2+(x-camera.x)*camera.zoom,screenY=height/2+(y-camera.y)*camera.zoom;
      assert.ok(screenX>=19.99&&screenX<=width-19.99);
      assert.ok(screenY>=19.99&&screenY<=height-19.99);
    }
  }
  for(const n of Object.values(fitCamera(boundsOf([]),0,0)))assert.ok(Number.isFinite(n));
});

test('cursor-anchored zoom preserves its world point and obeys zoom limits',()=>{
  const camera={x:300,y:-200,zoom:.2},anchor={x:190,y:-80},next=zoomCamera(camera,1.5,anchor);
  assert.ok(Math.abs(camera.x+anchor.x/camera.zoom-next.x-anchor.x/next.zoom)<1e-9);
  assert.ok(Math.abs(camera.y+anchor.y/camera.zoom-next.y-anchor.y/next.zoom)<1e-9);
  assert.equal(zoomCamera(camera,1e9).zoom,8);
  assert.equal(zoomCamera(camera,1e-9).zoom,.005);
});

test('pinning preserves existing positions and places new atoms outside them',()=>{
  const initial=arrangeGraph(['0','1'],[edge('0','1')],false);
  const positions=new Map(initial.nodes.map(n=>[n.id,{x:n.x,y:n.y}]));
  const pinned=arrangeGraph(['0','1','2'],[edge('0','1'),edge('1','2')],false,positions,true);
  assert.deepEqual(pinned.nodes.slice(0,2),initial.nodes);
  assert.ok(pinned.nodes[2].x>boundsOf(initial.nodes).maxX);
});

test('parallel, reverse and loop fields keep distinct paths and identities',()=>{
  const nodes=[{id:'a',x:0,y:0},{id:'b',x:200,y:0}];
  const links=[edge('a','b','ab1'),edge('a','b','ab2'),edge('b','a','ba'),edge('a','a','aa1'),edge('a','a','aa2')];
  const routed=routeLinks(nodes,links,false);
  assert.equal(new Set(routed.map(l=>l.path)).size,5);
  assert.deepEqual(new Set(routed.map(l=>l.id)),new Set(links.map(l=>l.id)));
  const bounds=boundsOf([...nodes,...routed.flatMap(l=>l.extent)]);
  assert.ok(bounds.minY<-100,'fit includes loop control points, not only node centres');
});

test('focus prioritises a late identity beyond the overview limit and expands by hops',()=>{
  const ids=Array.from({length:600},(_,i)=>String(i));
  const links=ids.slice(0,-1).map((id,i)=>edge(id,String(i+1)));
  assert.deepEqual(neighbourhood(ids,links,'573',1),['573','572','574']);
  assert.deepEqual(neighbourhood(ids,links,'573',2),['573','572','574','571','575']);
  assert.deepEqual(neighbourhood(ids,links,'missing',2),[]);
  const star=ids.slice(0,-1).map(id=>edge(id,'599'));
  assert.equal(neighbourhood(ids,star,'599',1).slice(0,250)[0],'599');
});

test('empty, singleton and practical layouts remain finite and input-preserving',()=>{
  assert.deepEqual(arrangeGraph([],[],false),{nodes:[],components:[]});
  const lone=arrangeGraph(['a'],[edge('a','a')],true);
  assert.equal(lone.nodes.length,1);
  assert.ok(Number.isFinite(lone.nodes[0].x)&&Number.isFinite(lone.nodes[0].y));
});
