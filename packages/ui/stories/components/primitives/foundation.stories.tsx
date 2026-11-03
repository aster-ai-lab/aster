import type { Meta, StoryObj } from '@storybook/react'
import { useState } from 'react'

import {
  Accordion, AccordionContent, AccordionItem, AccordionTrigger,
  Alert, Badge, Button, Checkbox,
  Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList,
  ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger,
  Dialog, DialogContent, DialogDescription, DialogTitle, DialogTrigger,
  Drawer, DrawerContent, DrawerDescription, DrawerTitle, DrawerTrigger,
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSub,
  DropdownMenuSubContent, DropdownMenuSubTrigger, DropdownMenuTrigger,
  HoverCard, HoverCardContent, HoverCardTrigger,
  Input, InputGroup, InputGroupAddon, InputGroupInput, InputGroupText,
  Label, Popover, PopoverContent, PopoverTrigger, RadioGroup, RadioGroupItem,
  SegmentedControl, Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
  Separator, Skeleton, Slider, Switch, Table, TableBody, TableCell, TableHead,
  TableHeader, TableRow, Tabs, TabsContent, TabsList, TabsTrigger, Textarea, Tooltip
} from '../../../src'
import { SearchSelect } from '../../../src/components/console/SearchSelect'
import { Combobox } from '../../../src/components/primitives/combobox'

import '../../../src/components/arc/foundation.css'

const options = [
  { value: 'alpha', label: 'Alpha', group: 'Letters' },
  { value: 'beta', label: 'Beta', group: 'Letters' },
  { value: 'gamma', label: 'Gamma unavailable', disabled: true, group: 'Letters' }
]

/** Real shared controls with local owners; no substitute widget or control styling. */
export function FoundationReview() {
  const [choice, setChoice] = useState('alpha')
  const [actions, setActions] = useState(0)
  const [searchChoice, setSearchChoice] = useState('alpha')
  const [selectChoice, setSelectChoice] = useState('alpha')
  const [scrollChoice, setScrollChoice] = useState('0')
  const [comboChoice, setComboChoice] = useState<string | string[]>('alpha')
  const choose = (value: string) => { setChoice(value); setActions((n) => n + 1) }
  return (
    <main className="mx-auto grid max-w-6xl gap-6 p-8 text-[13px] text-foreground">
      <header>
        <h1 className="text-2xl font-semibold">Arc foundations · Aster compact</h1>
        <p className="mt-2 text-muted-foreground">Shared controls, their states, and their actual popup owners.</p>
        <output data-testid="choice">Choice: {choice}</output>
        <output data-testid="actions" className="ml-6">Actions: {actions}</output>
      </header>
      <div className="grid gap-8 md:grid-cols-2">
        <section data-testid="fields" className="grid content-start gap-3">
          <h2 className="text-base font-medium">Fields</h2>
          <Input aria-label="Default field" placeholder="Name" />
          <Input aria-label="Compact field" size="sm" placeholder="Search" />
          <Input aria-label="Invalid field" aria-invalid="true" defaultValue="Invalid value" />
          <Input aria-label="Disabled field" disabled defaultValue="Unavailable" />
          <InputGroup><InputGroupAddon><InputGroupText>https://</InputGroupText></InputGroupAddon><InputGroupInput aria-label="Grouped field" placeholder="example.com" /></InputGroup>
          <Textarea.Input aria-label="Notes" defaultValue="The field keeps its place when focus or validation changes." />
        </section>
        <section data-testid="buttons" className="grid content-start gap-3">
          <h2 className="text-base font-medium">Actions</h2>
          <div className="flex flex-wrap gap-3">
            <Button>Primary action</Button><Button variant="outline">Outline action</Button>
            <Button variant="secondary">Quiet action</Button><Button variant="ghost">Ghost action</Button>
            <Button variant="destructive">Delete action</Button><Button disabled>Disabled action</Button>
          </div>
          <div data-testid="equivalent-buttons" className="flex flex-wrap gap-3">
            <Button variant="secondary">Native action</Button>
            <Button variant="secondary" loading={false} loadingIcon={<span>…</span>}>Custom loader action</Button>
            <Button variant="secondary" href="#anchor">Anchor action</Button>
            <Button variant="secondary" asChild><a href="#slot">Slotted action</a></Button>
          </div>
          <div className="flex gap-3"><Button loading>Pending action</Button><Button loading loadingIcon={<span>…</span>}>Custom pending</Button></div>
          <Button href="#blocked" loading loadingIcon={<span>…</span>} onClick={() => choose('blocked')}>Blocked link</Button>
        </section>
        <section data-testid="toggles" className="grid content-start gap-4">
          <h2 className="text-base font-medium">Choices</h2>
          <div className="flex items-center gap-3"><Checkbox id="review-checkbox" defaultChecked /><Label htmlFor="review-checkbox">Checkbox</Label><Switch aria-label="Switch" defaultChecked /><Switch aria-label="Disabled switch" disabled /></div>
          <RadioGroup defaultValue="one" aria-label="Radio choices" className="flex gap-4"><Label><RadioGroupItem value="one" />One</Label><Label><RadioGroupItem value="two" />Two</Label></RadioGroup>
          <SegmentedControl aria-label="View" options={[{ value: 'overview', label: 'Overview' }, { value: 'detail', label: 'Detail' }]} />
          <Tabs defaultValue="one"><TabsList><TabsTrigger value="one">First tab</TabsTrigger><TabsTrigger value="two">Second tab</TabsTrigger></TabsList><TabsContent value="one">First panel</TabsContent><TabsContent value="two">Second panel</TabsContent></Tabs>
          <Slider defaultValue={[44]} getThumbAriaLabel={() => 'Amount'} />
        </section>
        <section data-testid="pickers" className="grid content-start gap-3">
          <h2 className="text-base font-medium">Pickers</h2>
          <SearchSelect aria-label="Search choices" options={options} value={searchChoice} onChange={setSearchChoice} allowClear={false} />
          <Select value={selectChoice} onValueChange={setSelectChoice}><SelectTrigger aria-label="Select choices"><SelectValue /></SelectTrigger><SelectContent>{options.map((o) => <SelectItem key={o.value} value={o.value} disabled={o.disabled}>{o.label}</SelectItem>)}</SelectContent></Select>
          <Combobox aria-label="Combobox choices" options={options} value={comboChoice} onChange={setComboChoice} />
          <SearchSelect aria-label="Scrollable choices" options={Array.from({ length: 24 }, (_,i) => ({ value: String(i), label: 'Item ' + (i + 1) }))} value={scrollChoice} onChange={setScrollChoice} allowClear={false} />
          <output data-testid="search-choice">Search: {searchChoice}</output>
          <output data-testid="select-choice">Select: {selectChoice}</output>
        </section>
        <section data-testid="overlays" className="grid content-start gap-3">
          <h2 className="text-base font-medium">Overlays</h2>
          <div className="flex flex-wrap gap-3">
            <DropdownMenu><DropdownMenuTrigger asChild><Button variant="outline">Open menu</Button></DropdownMenuTrigger><DropdownMenuContent>
              <DropdownMenuItem onSelect={() => choose('alpha')}>Alpha action</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => choose('beta')}>Beta action</DropdownMenuItem>
              <DropdownMenuItem disabled onSelect={() => choose('blocked')}>Unavailable action</DropdownMenuItem>
              <DropdownMenuSub><DropdownMenuSubTrigger>More actions</DropdownMenuSubTrigger><DropdownMenuSubContent><DropdownMenuItem onSelect={() => choose('nested')}>Nested action</DropdownMenuItem></DropdownMenuSubContent></DropdownMenuSub>
              <DropdownMenuItem variant="destructive" onSelect={() => choose('delete')}>Delete item</DropdownMenuItem>
            </DropdownMenuContent></DropdownMenu>
            <Popover><PopoverTrigger asChild><Button variant="outline">Open choices</Button></PopoverTrigger><PopoverContent className="grid gap-1 p-2"><Button variant="ghost" data-popup-item="" onClick={() => choose('alpha')}>Alpha choice</Button><Button variant="ghost" data-popup-item="" onClick={() => choose('beta')}>Beta choice</Button></PopoverContent></Popover>
            <Dialog><DialogTrigger asChild><Button variant="outline">Open dialog</Button></DialogTrigger><DialogContent><DialogTitle>Review dialog</DialogTitle><DialogDescription>Focus, geometry and motion belong to the shared dialog.</DialogDescription><Input aria-label="Dialog field" placeholder="Name" /><Button onClick={() => choose('dialog')}>Dialog action</Button></DialogContent></Dialog>
            <Drawer><DrawerTrigger asChild><Button variant="outline">Open drawer</Button></DrawerTrigger><DrawerContent><div className="grid gap-3 p-6"><DrawerTitle>Review drawer</DrawerTitle><DrawerDescription>Uses the existing drawer owner.</DrawerDescription><Input aria-label="Drawer field" /></div></DrawerContent></Drawer>
            <Tooltip content="Helpful detail" delay={0}><Button variant="ghost">Tooltip trigger</Button></Tooltip>
            <HoverCard><HoverCardTrigger asChild><Button variant="ghost">Hover preview</Button></HoverCardTrigger><HoverCardContent>Preview content</HoverCardContent></HoverCard>
          </div>
          <ContextMenu><ContextMenuTrigger className="grid h-24 place-items-center rounded-lg border border-dashed" data-testid="context-target">Right click for actions</ContextMenuTrigger><ContextMenuContent><ContextMenuItem onSelect={() => choose('alpha')}>Alpha context</ContextMenuItem><ContextMenuItem onSelect={() => choose('beta')}>Beta context</ContextMenuItem></ContextMenuContent></ContextMenu>
        </section>
        <section data-testid="commands" className="grid content-start gap-3">
          <h2 className="text-base font-medium">Command list</h2>
          <Command><CommandInput placeholder="Filter commands" /><CommandList><CommandEmpty>No matching commands</CommandEmpty><CommandGroup heading="Actions"><CommandItem onSelect={() => choose('alpha')}>Alpha command</CommandItem><CommandItem onSelect={() => choose('beta')}>Beta command</CommandItem><CommandItem disabled>Unavailable command</CommandItem></CommandGroup></CommandList></Command>
        </section>
      </div>
      <Separator />
      <section data-testid="feedback" className="grid gap-4 md:grid-cols-2">
        <div className="grid gap-3"><Alert description="A readable status stays in the same place." /><div className="flex gap-2"><Badge>Default</Badge><Badge variant="outline">Outline</Badge></div><Skeleton className="h-8 w-48" /></div>
        <div><Accordion type="single" collapsible><AccordionItem value="details"><AccordionTrigger>Supporting details</AccordionTrigger><AccordionContent>Expansion retains its trigger and reading order.</AccordionContent></AccordionItem></Accordion><Table><TableHeader><TableRow><TableHead>Item</TableHead><TableHead>State</TableHead></TableRow></TableHeader><TableBody><TableRow><TableCell>Shared control</TableCell><TableCell>Ready</TableCell></TableRow></TableBody></Table></div>
      </section>
    </main>
  )
}

const meta = { title: 'Components/Primitives/Arc foundation review', component: FoundationReview } satisfies Meta<typeof FoundationReview>
export default meta
type Story = StoryObj<typeof meta>
export const AllStates: Story = {}
