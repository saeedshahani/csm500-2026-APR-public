import {deleteApp,initializeApp} from 'firebase/app';
import {connectAuthEmulator,createUserWithEmailAndPassword,initializeAuth,inMemoryPersistence,signInWithEmailAndPassword,signOut} from 'firebase/auth';

export const emulatorProject='demo-reflective-graph';
export type EmulatorSession={uid:string;token:()=>Promise<string>;close:()=>Promise<void>};

export async function emulatorSession(email:string,password:string,create:boolean):Promise<EmulatorSession>{
  const app=initializeApp({projectId:emulatorProject,apiKey:'emulator-only'},`graph-${crypto.randomUUID()}`);
  const auth=initializeAuth(app,{persistence:inMemoryPersistence});
  // Projects renders a scoped warning; the SDK's fixed banner obscures controls and survives sign-out.
  connectAuthEmulator(auth,'http://127.0.0.1:9099',{disableWarnings:true});
  let closed=false;
  const close=async()=>{if(closed)return;closed=true;try{await signOut(auth);}finally{await deleteApp(app);}};
  try{
    const credentials=await (create?createUserWithEmailAndPassword:signInWithEmailAndPassword)(auth,email,password);
    return {uid:credentials.user.uid,token:async()=>{if(closed)throw Error('Signed out');return credentials.user.getIdToken();},close};
  }catch(error){await close();throw error;}
}
