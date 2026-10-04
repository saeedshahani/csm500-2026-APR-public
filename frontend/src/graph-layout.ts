import {forceSimulation, forceLink, forceManyBody, forceCenter, forceCollide, type SimulationNodeDatum} from 'd3-force';

export type Point = {x:number;y:number};
export type GraphNode = Point & {id:string};
export type GraphLink = {id:string;source:string;target:string;record:string;label:string};
export type Bounds = {minX:number;minY:number;maxX:number;maxY:number};
export type Camera = Point & {zoom:number};
export type GraphComponent = {ids:string[];bounds:Bounds};
export type Layout = {nodes:GraphNode[];components:GraphComponent[]};
type ForceNode = SimulationNodeDatum & GraphNode;

export const NODE_LIMIT = 250;
export const LINK_LIMIT = 500;

export function boundsOf(points:Point[], padding=80):Bounds {
  if(!points.length)return {minX:-padding,minY:-padding,maxX:padding,maxY:padding};
  return {
    minX:Math.min(...points.map(p=>p.x))-padding, minY:Math.min(...points.map(p=>p.y))-padding,
    maxX:Math.max(...points.map(p=>p.x))+padding, maxY:Math.max(...points.map(p=>p.y))+padding,
  };
}

/** Camera coordinates are world coordinates at the centre of the viewport. */
export function fitCamera(bounds:Bounds,width:number,height:number):Camera {
  return {
    x:(bounds.minX+bounds.maxX)/2, y:(bounds.minY+bounds.maxY)/2,
    zoom:Math.min(1.5,Math.max(1,width-40)/Math.max(1,bounds.maxX-bounds.minX),Math.max(1,height-40)/Math.max(1,bounds.maxY-bounds.minY)),
  };
}

/** Keep a cursor's world position fixed while zooming. */
export function zoomCamera(camera:Camera,factor:number,anchor:Point={x:0,y:0}):Camera {
  const zoom=Math.min(8,Math.max(.005,camera.zoom*factor));
  return {zoom,x:camera.x+anchor.x/camera.zoom-anchor.x/zoom,y:camera.y+anchor.y/camera.zoom-anchor.y/zoom};
}

/** Weak components of the displayed graph only, not a semantic decomposition. */
export function connectedComponents(ids:string[],links:GraphLink[]):string[][] {
  const neighbours=new Map(ids.map(id=>[id,new Set<string>()]));
  for(const link of links){neighbours.get(link.source)?.add(link.target);neighbours.get(link.target)?.add(link.source);}
  const seen=new Set<string>(),components:string[][]=[];
  for(const id of ids){
    if(seen.has(id))continue;
    const group=[id];seen.add(id);
    for(let i=0;i<group.length;i++)for(const next of neighbours.get(group[i])??[]){
      if(neighbours.has(next)&&!seen.has(next)){seen.add(next);group.push(next);}
    }
    components.push(group);
  }
  return components;
}

/** Settle off-screen, then pack components with whitespace. No viewport clamping:
 * the camera fits the drawing, rather than compressing the drawing into a box.
 * The bounded input keeps this synchronous layout independent of the engine. */
export function arrangeGraph(ids:string[],links:GraphLink[],practical:boolean,previous=new Map<string,Point>(),pinned=false):Layout {
  const components=connectedComponents(ids,links);
  if(pinned&&previous.size){
    const existing=ids.filter(id=>previous.has(id)).map(id=>({id,...previous.get(id)!}));
    const edge=existing.length?boundsOf(existing).maxX:0;
    const fresh=ids.filter(id=>!previous.has(id)).map((id,i)=>({id,x:edge+140+(i%8)*130,y:Math.floor(i/8)*140}));
    const nodes=[...existing,...fresh],byId=new Map(nodes.map(n=>[n.id,n]));
    return {nodes,components:components.map(group=>({ids:group,bounds:boundsOf(group.map(id=>byId.get(id)!))}))};
  }
  const layouts=components.map(group=>{
    const members=new Set(group);
    const nodes:ForceNode[]=group.map((id,i)=>{
      const radius=85*Math.sqrt(i+1),angle=i*2.399963;
      // Unpinned layout depends only on topology, not the order of UI filters.
      return {id,x:radius*Math.cos(angle),y:radius*Math.sin(angle)};
    });
    const edges=links.filter(link=>members.has(link.source)&&members.has(link.target)).map(link=>({...link}));
    const simulation=forceSimulation(nodes).stop()
      .force('link',forceLink<ForceNode,GraphLink>(edges).id(n=>n.id).distance(practical?155:115).strength(.35))
      .force('charge',forceManyBody().strength(practical?-650:-400))
      .force('center',forceCenter(0,0))
      .force('collision',forceCollide(practical?60:46).strength(1).iterations(3));
    simulation.tick(220);
    simulation.stop();
    return {nodes,bounds:boundsOf(nodes,90)};
  });

  // Separate disconnected representations without adding semantic boundaries.
  const area=layouts.reduce((sum,l)=>sum+(l.bounds.maxX-l.bounds.minX)*(l.bounds.maxY-l.bounds.minY),0);
  const rowWidth=Math.max(600,Math.sqrt(area*1.65));
  let x=0,y=0,rowHeight=0;
  const nodes:GraphNode[]=[],groups:GraphComponent[]=[];
  for(const layout of layouts){
    const width=layout.bounds.maxX-layout.bounds.minX,height=layout.bounds.maxY-layout.bounds.minY;
    if(x>0&&x+width>rowWidth){x=0;y+=rowHeight+65;rowHeight=0;}
    const placed=layout.nodes.map(n=>({id:n.id,x:n.x-layout.bounds.minX+x,y:n.y-layout.bounds.minY+y}));
    nodes.push(...placed);groups.push({ids:placed.map(n=>n.id),bounds:boundsOf(placed,70)});
    x+=width+65;rowHeight=Math.max(rowHeight,height);
  }
  return {nodes,components:groups};
}

export type RoutedLink = GraphLink & {path:string;labelPoint:Point;extent:Point[]};
/** Group parallel/reverse fields once instead of searching all edges per frame. */
export function routeLinks(nodes:GraphNode[],links:GraphLink[],practical:boolean):RoutedLink[] {
  const byId=new Map(nodes.map(n=>[n.id,n]));
  const peers=new Map<string,GraphLink[]>();
  for(const link of links){const key=JSON.stringify([link.source,link.target].sort());const group=peers.get(key)??[];group.push(link);peers.set(key,group);}
  return [...peers.values()].flatMap(group=>group.flatMap((link,index)=>{
    const a=byId.get(link.source),b=byId.get(link.target);if(!a||!b)return [];
    const dx=b.x-a.x,dy=b.y-a.y,len=Math.hypot(dx,dy)||1;
    const end={x:b.x-dx/len*(practical?29:24),y:b.y-dy/len*(practical?29:24)};
    const bend=(index-(group.length-1)/2)*42*(a.id<b.id?1:-1);
    const control={x:(a.x+end.x)/2-dy/len*bend,y:(a.y+end.y)/2+dx/len*bend};
    if(a.id===b.id){
      const radius=55+index*25,c1={x:a.x+radius,y:a.y-radius-50},c2={x:a.x+radius,y:a.y+radius};
      return [{...link,path:`M ${a.x} ${a.y-18} C ${c1.x} ${c1.y}, ${c2.x} ${c2.y}, ${a.x+18} ${a.y}`,labelPoint:{x:a.x+radius,y:a.y-radius/2},extent:[a,c1,c2]}];
    }
    return [{...link,path:`M ${a.x} ${a.y} Q ${control.x} ${control.y} ${end.x} ${end.y}`,labelPoint:{x:(a.x+2*control.x+end.x)/4,y:(a.y+2*control.y+end.y)/4-7},extent:[a,control,end]}];
  }));
}

/** BFS order gives the selected identity priority even beyond the canvas cap. */
export function neighbourhood(ids:string[],links:{source:string;target:string}[],focus:string,depth:number):string[] {
  const allowed=new Set(ids),adjacency=new Map(ids.map(id=>[id,new Set<string>()]));
  for(const {source,target} of links)if(allowed.has(source)&&allowed.has(target)){
    adjacency.get(source)!.add(target);adjacency.get(target)!.add(source);
  }
  if(!allowed.has(focus))return [];
  const result=[focus],seen=new Set(result);let frontier=[focus];
  for(let i=0;i<depth;i++){
    const next:string[]=[];
    for(const id of frontier)for(const neighbour of adjacency.get(id)??[])if(!seen.has(neighbour)){seen.add(neighbour);result.push(neighbour);next.push(neighbour);}
    frontier=next;
  }
  return result;
}
