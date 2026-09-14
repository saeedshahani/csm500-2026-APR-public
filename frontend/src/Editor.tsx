import {useEffect,useRef} from 'react';
import {EditorState,Compartment} from '@codemirror/state';
import {EditorView,lineNumbers,highlightActiveLine,drawSelection,Decoration} from '@codemirror/view';
export function Editor({value,onChange,diagnostic,jump=0}:{value:string;onChange:(text:string)=>void;diagnostic?:{line:string|null;column:string|null;message:string}|null;jump?:number}) {
  const host=useRef<HTMLDivElement>(null),view=useRef<EditorView|null>(null),callback=useRef(onChange);
  const marks=useRef(new Compartment());
  useEffect(()=>{callback.current=onChange;},[onChange]);
  useEffect(()=>{
    if(!host.current)return;
    const editor=new EditorView({parent:host.current,state:EditorState.create({doc:value,extensions:[marks.current.of([]),lineNumbers(),highlightActiveLine(),drawSelection(),EditorView.lineWrapping,EditorView.contentAttributes.of({'aria-label':'Program source','spellcheck':'false'}),EditorView.updateListener.of(update=>{if(update.docChanged)callback.current(update.state.doc.toString());})]})});view.current=editor;
    return ()=>{editor.destroy();view.current=null;};
    // Initial content is followed by the controlled update below.
  },[]);
  useEffect(()=>{const editor=view.current;if(editor&&editor.state.doc.toString()!==value)editor.dispatch({changes:{from:0,to:editor.state.doc.length,insert:value}});},[value]);
  useEffect(()=>{const editor=view.current;if(!editor)return;const line=diagnostic?.line?editor.state.doc.line(Math.max(1,Math.min(editor.state.doc.lines,Number(diagnostic.line)))):null;editor.dispatch({effects:marks.current.reconfigure(line?EditorView.decorations.of(Decoration.set([Decoration.line({class:'source-error',attributes:{title:diagnostic!.message}}).range(line.from)])):[])});},[diagnostic,value]);
  useEffect(()=>{const editor=view.current;if(!editor||!diagnostic?.line||!jump)return;const line=editor.state.doc.line(Math.min(editor.state.doc.lines,Number(diagnostic.line)));const offset=Array.from(line.text).slice(0,Math.max(0,Number(diagnostic.column??'1')-1)).join('').length;editor.dispatch({selection:{anchor:line.from+offset},scrollIntoView:true});editor.focus();},[jump,diagnostic]);
  return <div className="source-editor" ref={host}/>;
}
