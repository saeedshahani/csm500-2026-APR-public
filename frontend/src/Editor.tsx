import {useEffect,useMemo,useRef} from 'react';
import {EditorState,Compartment,Annotation} from '@codemirror/state';
import {EditorView,lineNumbers,highlightActiveLine,drawSelection,Decoration,keymap,WidgetType} from '@codemirror/view';
import {defaultKeymap,history,historyKeymap} from '@codemirror/commands';
import type {SourceError} from './types';
import {locateDiagnostics} from './diagnostics';
const controlledUpdate=Annotation.define<boolean>();
class InsertionError extends WidgetType {
  readonly message:string;readonly related:boolean;
  constructor(message:string,related:boolean){super();this.message=message;this.related=related;}
  toDOM(){const span=document.createElement('span');span.className=this.related?'source-related':'source-error';span.textContent='▏';span.title=this.message;span.setAttribute('role','img');span.setAttribute('aria-label','Diagnostic at insertion point');return span;}
}
export function EditorDiagnostics({source,error,onJump}:{source:string;error:SourceError|null;onJump:(key:string)=>void}){
 const locations=useMemo(()=>locateDiagnostics(source,error?.diagnostics),[source,error]);
 if(!error)return null;
 const entries=error.diagnostics?.flatMap((e,i)=>[{key:String(i),message:e.message,related:false},...e.related.map((r,j)=>({key:`${i}:${j}`,message:r.message,related:true}))])??[];
 return <div className="source-diagnostics" role="status" aria-label="Source diagnostics">{entries.length?<ol>{entries.map(e=>{const location=locations.find(l=>l.key===e.key);return <li key={e.key}>{e.related?'Related location':'Error'}: {e.message} {location?<button onClick={()=>onJump(e.key)}>Go to line {location.line}, column {location.column}{e.related?' (related)':''}</button>:<span>Source location unavailable.</span>}</li>;})}</ol>:<p>{error.message}</p>}</div>;
}
export function Editor({value,onChange,diagnostic,jump=0,target=null}:{value:string;onChange:(text:string)=>void;diagnostic?:SourceError|null;jump?:number;target?:string|null}) {
  const host=useRef<HTMLDivElement>(null),view=useRef<EditorView|null>(null),callback=useRef(onChange);
  const marks=useRef(new Compartment());
  const locations=useMemo(()=>locateDiagnostics(value,diagnostic?.diagnostics),[value,diagnostic]);
  useEffect(()=>{callback.current=onChange;},[onChange]);
  useEffect(()=>{
    if(!host.current)return;
    const editor=new EditorView({parent:host.current,state:EditorState.create({doc:value,extensions:[history(),keymap.of([...defaultKeymap,...historyKeymap]),marks.current.of([]),lineNumbers(),highlightActiveLine(),drawSelection(),EditorView.lineWrapping,EditorView.contentAttributes.of({'aria-label':'Program source','spellcheck':'false'}),EditorView.updateListener.of(update=>{if(update.docChanged&&!update.transactions.every(t=>t.annotation(controlledUpdate)))callback.current(update.state.doc.toString());})]})});view.current=editor;
    return ()=>{editor.destroy();view.current=null;};
    // Initial content is followed by the controlled update below.
  },[]);
  useEffect(()=>{const editor=view.current;if(editor&&editor.state.doc.toString()!==value)editor.dispatch({changes:{from:0,to:editor.state.doc.length,insert:value},annotations:controlledUpdate.of(true)});},[value]);
  useEffect(()=>{const editor=view.current;if(!editor)return;const ranges=locations.filter(e=>e.to<=editor.state.doc.length).map(e=>e.from===e.to?Decoration.widget({widget:new InsertionError(e.message,e.related),side:1}).range(e.from):Decoration.mark({class:e.related?'source-related':'source-error',attributes:{title:e.message}}).range(e.from,e.to));editor.dispatch({effects:marks.current.reconfigure(EditorView.decorations.of(Decoration.set(ranges,true)))});},[locations]);
  useEffect(()=>{const editor=view.current;const location=locations.find(e=>e.key===target);if(!editor||!location||!jump||location.to>editor.state.doc.length)return;editor.dispatch({selection:{anchor:location.from,head:location.to},scrollIntoView:true});editor.focus();},[jump,locations,target]);
  return <div className="source-editor" ref={host}/>;
}
