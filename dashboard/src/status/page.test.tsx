// @vitest-environment jsdom
import { cleanup, render, screen, fireEvent, waitFor } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { StatusPage } from './page';
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
const config={udp:{port:20777},http:{port:8777,bind:'0.0.0.0'},forward:{enabled:false,targets:[]},recording:{enabled:true,size_cap_gb:20},app:{start_with_windows:false}};
function server(host:boolean) {
 const requests:{path:string,body:unknown}[]=[];
 vi.stubGlobal('fetch',vi.fn(async(path:string,options?:RequestInit)=> {
   requests.push({path,body:options?.body ? JSON.parse(String(options.body)):null});
   return {ok:true,json:async()=> path==='/api/status' ? {host,collector:{packetRateHz:20,lastFormat:2024,dropped:3,uidZero:4,closedUid:5},urls:['http://192.168.1.5:8777'],missingStreamLayouts:['Broadcast'],track:[7,7004]} : path==='/api/recordings' ? [{name:'race.pitrec',size:42,sidecar:{pinned:false,track:'Spa',duration_us:60000000,laps:1}}] : path==='/api/settings' ? config : {redirect:null}};
 }));return requests;
}
test('LAN status shows setup, format, missing layout and notice but no host controls',async()=>{
 server(false);render(<StatusPage/>);
 await screen.findByText('Broadcast');
 expect(screen.getByText(/Unsupported telemetry format 2024/)).toBeTruthy();
 expect(screen.getByText(/below 60 Hz/)).toBeTruthy();
 expect(screen.getByText(/Formula One Licensing B.V./)).toBeTruthy();
 expect(screen.getByText('race.pitrec')).toBeTruthy();
 expect(screen.queryByRole('button',{name:'Save settings'})).toBeNull();
 expect(screen.queryByRole('button',{name:'Delete'})).toBeNull();
});
test('host can pin, rescan, relearn and save settings',async()=>{
 const requests=server(true);render(<StatusPage/>);
 await screen.findByRole('button',{name:'Save settings'});
 fireEvent.click(screen.getByRole('button',{name:'Pin'}));
 await waitFor(()=>expect((screen.getByRole('button',{name:'Pin'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'Rescan recordings'}));
 await waitFor(()=>expect((screen.getByRole('button',{name:'Rescan recordings'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'Relearn map'}));
 await waitFor(()=>expect((screen.getByRole('button',{name:'Relearn map'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.change(screen.getByLabelText('UDP port'),{target:{value:'23456'}});
 fireEvent.click(screen.getByRole('button',{name:'Save settings'}));
 await waitFor(()=>expect(requests).toContainEqual({path:'/api/settings',body:{...config,udp:{port:23456}}}));
 expect(requests).toContainEqual({path:'/api/recordings/race.pitrec/pin',body:{pinned:true}});
 expect(requests).toContainEqual({path:'/api/tracks/7/7004/relearn',body:{}});
});

test('host cancels an HTTP listener change before it is saved', async () => {
  const requests = server(true);
  vi.spyOn(window, 'confirm').mockReturnValue(false);
  render(<StatusPage />);
  await screen.findByRole('button', { name: 'Save settings' });
  fireEvent.change(screen.getByLabelText('HTTP port'), { target: { value: '9876' } });
  fireEvent.click(screen.getByRole('button', { name: 'Save settings' }));
  await waitFor(() => expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('disconnects every screen')));
  expect(requests.filter((r) => r.path === '/api/settings' && r.body !== null)).toEqual([]);
  vi.restoreAllMocks();
});
