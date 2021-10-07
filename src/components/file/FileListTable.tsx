import clsx from 'clsx';
import React, { useMemo, useState } from 'react';
import { useTable } from 'react-table';

const columns = [
  { Header: 'Name', accessor: 'name' },
  { Header: 'Size', accessor: 'size' }
];

export const dummyFileData = [
  { id: 1, name: 'MyNameJeff.mp4', type: 'video', size: '5GB' },
  { id: 2, name: 'cow.ogg', type: 'audio', size: '345KB' },
  { id: 3, name: 'calvin.fig' },
  { id: 4, name: 'calvin.fig' },
  { id: 5, name: 'calvin.fig' },
  { id: 6, name: 'calvin.fig' }
];

export const FileListTable: React.FC<{}> = (props) => {
  // @ts-expect-error
  const tableInstance = useTable({ columns, data: dummyFileData });
  const [selectedRow, setSelectedRow] = useState(0);
  const { getTableProps, getTableBodyProps, headerGroups, rows, prepareRow } = tableInstance;

  return (
    <div className="w-full bg-gray-900  p-2 rounded-lg">
      <table className="w-full rounded-lg shadow-lg border-collapse" {...getTableProps()}>
        <thead>
          {headerGroups.map((headerGroup) => (
            <tr {...headerGroup.getHeaderGroupProps()}>
              {headerGroup.headers.map((column) => (
                <th
                  className="px-3 pt-2 text-left text-gray-500  text-sm"
                  {...column.getHeaderProps()}
                >
                  {column.render('Header')}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody {...getTableBodyProps()}>
          {rows.map((row) => {
            prepareRow(row);
            return (
              <tr
                onClick={() => setSelectedRow(row.original.id || 0)}
                className={clsx('even:bg-[#00000040] border-2 border-opacity-0 rounded-sm', {
                  'border-2 border-opacity-100 border-primary z-50': selectedRow === row.original.id
                })}
                {...row.getRowProps()}
              >
                {row.cells.map((cell) => {
                  return (
                    <td className={clsx('py-2 px-4 rounded-sm')} {...cell.getCellProps()}>
                      {cell.render('Cell')}
                    </td>
                  );
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};
