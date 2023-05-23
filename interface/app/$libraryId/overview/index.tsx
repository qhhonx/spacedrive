import { useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { z } from '@sd/ui/src/forms';
import { Category } from '~/../packages/client/src';
import { useExplorerTopBarOptions } from '~/hooks';
import Explorer from '../Explorer';
import { SEARCH_PARAMS } from '../Explorer/util';
import { usePageLayout } from '../PageLayout';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

export type SearchArgs = z.infer<typeof SEARCH_PARAMS>;

export const Component = () => {
	const page = usePageLayout();
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, query } = useItems(selectedCategory);

	return (
		<>
			<TopBarPortal
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>
			<Statistics />
			<Categories selected={selectedCategory} onSelectedChanged={setSelectedCategory} />
			<Explorer
				inspectorClassName="!pt-0 !fixed !top-[50px] !right-[10px] !w-[260px]"
				viewClassName="!pl-0 !pt-[0] !h-auto !overflow-visible"
				listViewHeadersClassName="!top-[65px] z-30"
				items={items}
				onLoadMore={query.fetchNextPage}
				hasNextPage={query.hasNextPage}
				isFetchingNextPage={query.isFetchingNextPage}
				scrollRef={page?.ref}
			/>
		</>
	);
};
